use crate::{config::config, redis};
use eyre::{Context, Result};
use log::debug;
use rand::{distributions::Alphanumeric, Rng};
use reqwest::Client;
use serde::Deserialize;
use url::Url;

pub fn get_callback_url(url: &Url) -> Result<Url> {
    Ok(url.join(&config().auth_callback)?)
}

pub fn create_login_url(original_url: &Url, callback_url: &Url) -> Result<Url> {
    Ok(Url::parse_with_params(
        config().auth_url.as_str(),
        &[
            ("client_id", config().client_id.as_str()),
            ("redirect_uri", callback_url.as_str()),
            ("response_type", "code"),
            ("scope", "openid"),
            ("state", original_url.as_str()),
        ],
    )?)
}

#[derive(Debug)]
pub struct CodeAuth {
    pub code: String,
    pub callback_url: Url,
}

#[derive(Debug)]
pub enum AuthType {
    Code(CodeAuth),
    RefreshToken(String),
}

#[derive(Deserialize, Debug)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: usize,
    pub refresh_expires_in: usize,
}

pub async fn get_token(auth: &AuthType) -> Result<TokenResponse> {
    let mut form = vec![
        ("client_id", config().client_id.as_str()),
        ("client_secret", config().client_secret.as_str()),
    ];
    match auth {
        AuthType::Code(CodeAuth { code, callback_url }) => {
            form.push(("grant_type", "authorization_code"));
            form.push(("code", code));
            form.push(("redirect_uri", callback_url.as_str()));
        }
        AuthType::RefreshToken(token) => {
            form.push(("grant_type", "refresh_token"));
            form.push(("refresh_token", token));
        }
    }
    Ok(Client::new()
        .post(config().token_url.clone())
        .form(&form)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

#[derive(Deserialize, Debug)]
pub struct UserInfo {
    #[serde(default)]
    pub roles: Vec<String>,
}

pub async fn get_userinfo(access_token: &str) -> Result<UserInfo> {
    Ok(Client::new()
        .get(config().userinfo_url.clone())
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await?
        .json()
        .await?)
}

#[derive(Debug)]
pub struct Session {
    pub session_id: String,
    pub userinfo: UserInfo,
}

pub async fn create_session(auth: CodeAuth) -> Result<Session> {
    let token = get_token(&AuthType::Code(auth))
        .await
        .wrap_err("could not fetch access token")?;
    let userinfo = get_userinfo(&token.access_token)
        .await
        .wrap_err("could not fetch user info")?;
    let session_id: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    redis::set_token(session_id.as_str(), &token)
        .await
        .wrap_err("could not store token in redis")?;
    Ok(Session {
        session_id,
        userinfo,
    })
}

pub async fn get_session(session_id: &str) -> Result<Session> {
    let token = redis::get_token(session_id)
        .await
        .wrap_err("could not fetch token from redis")?;
    let userinfo = match get_userinfo(&token.access_token).await {
        Ok(userinfo) => userinfo,
        Err(err) => {
            debug!("could not use access token to fetch userinfo: {:?}", err);
            let token_response = get_token(&AuthType::RefreshToken(token.refresh_token))
                .await
                .wrap_err("could not refresh access token")?;
            let userinfo = get_userinfo(&token_response.access_token)
                .await
                .wrap_err("could not use fresh access token to fetch userinfo")?;
            redis::set_token(session_id, &token_response)
                .await
                .wrap_err("could not store token in redis")?;
            userinfo
        }
    };
    Ok(Session {
        session_id: session_id.into(),
        userinfo,
    })
}

pub async fn is_authorized(session_id: &str, role: &str) -> Result<bool> {
    Ok(
        match redis::get_session_cache(session_id, role)
            .await
            .wrap_err("could not get session cache from redis")?
        {
            redis::SessionCache::Allowed => true,
            redis::SessionCache::Forbidden => false,
            redis::SessionCache::NotCached => {
                let result: bool = get_session(session_id)
                    .await
                    .wrap_err("could not fetch session data")?
                    .userinfo
                    .roles
                    .contains(&role.into());
                redis::update_session_cache(
                    session_id,
                    role,
                    if result {
                        &redis::SessionCache::Allowed
                    } else {
                        &redis::SessionCache::Forbidden
                    },
                )
                .await
                .wrap_err("could not update redis session cache")?;
                result
            }
        },
    )
}
