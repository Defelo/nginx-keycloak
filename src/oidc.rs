use crate::{config::config, redis};
use rand::{distributions::Alphanumeric, Rng};
use reqwest::Client;
use serde::Deserialize;
use url::Url;

pub fn get_callback_url(url: &Url) -> Option<Url> {
    url.join(&config().auth_callback).ok()
}

pub fn create_redirect_url(redirect_url: &Url, callback_url: &Url) -> Option<String> {
    Some(
        Url::parse_with_params(
            config().auth_url.as_str(),
            &[
                ("client_id", config().client_id.as_str()),
                ("redirect_uri", callback_url.as_str()),
                ("response_type", "code"),
                ("scope", "openid"),
                ("state", redirect_url.as_str()),
            ],
        )
        .ok()?
        .as_str()
        .into(),
    )
}

#[derive(Debug)]
pub struct CodeAuth {
    pub code: String,
    pub callback_url: String,
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

pub async fn get_token(auth: &AuthType) -> Option<TokenResponse> {
    let mut form = vec![
        ("client_id", config().client_id.as_str()),
        ("client_secret", config().client_secret.as_str()),
    ];
    match auth {
        AuthType::Code(CodeAuth { code, callback_url }) => {
            form.push(("grant_type", "authorization_code"));
            form.push(("code", code));
            form.push(("redirect_uri", callback_url));
        }
        AuthType::RefreshToken(token) => {
            form.push(("grant_type", "refresh_token"));
            form.push(("refresh_token", token));
        }
    }
    Client::new()
        .post(config().token_url.clone())
        .form(&form)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?
}

#[derive(Deserialize, Debug)]
pub struct UserInfo {
    #[serde(default)]
    pub roles: Vec<String>,
}

pub async fn get_userinfo(access_token: &str) -> Option<UserInfo> {
    Client::new()
        .get(config().userinfo_url.clone())
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?
}

#[derive(Debug)]
pub struct Session {
    pub session_id: String,
    pub userinfo: UserInfo,
}

pub async fn create_session(auth: CodeAuth) -> Option<Session> {
    let token = get_token(&AuthType::Code(auth)).await?;
    let userinfo = get_userinfo(&token.access_token).await?;
    let session_id: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    redis::set_token(session_id.as_str(), &token).await.ok()?;
    Some(Session {
        session_id,
        userinfo,
    })
}

pub async fn get_session(session_id: &str) -> Option<Session> {
    let token = redis::get_token(session_id).await.ok()?;
    let userinfo = match get_userinfo(&token.access_token).await {
        Some(userinfo) => userinfo,
        None => {
            let token_response = get_token(&AuthType::RefreshToken(token.refresh_token)).await?;
            let userinfo = get_userinfo(&token_response.access_token).await?;
            redis::set_token(session_id, &token_response).await.ok()?;
            userinfo
        }
    };
    Some(Session {
        session_id: session_id.into(),
        userinfo,
    })
}

pub async fn is_authorized(session_id: &str, role: &str) -> Option<bool> {
    Some(
        match redis::get_session_cache(session_id, role).await.ok()? {
            redis::SessionCache::Allowed => true,
            redis::SessionCache::Forbidden => false,
            redis::SessionCache::NotCached => {
                let result: bool = get_session(session_id)
                    .await?
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
                .ok()?;
                result
            }
        },
    )
}
