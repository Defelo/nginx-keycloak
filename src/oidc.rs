use eyre::{Context, Result};
use log::debug;
use rand::{distributions::Alphanumeric, Rng};
use reqwest::Client;
use serde::Deserialize;
use url::Url;

use crate::redis::{Redis, SessionCache};

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

pub struct OIDC {
    auth_url: Url,
    token_url: Url,
    userinfo_url: Url,
    client_id: String,
    client_secret: String,
    pub auth_callback_path: String,
    redis: Redis,
}

#[derive(Deserialize, Debug)]
pub struct UserInfo {
    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug)]
pub struct Session {
    pub session_id: String,
    pub userinfo: UserInfo,
}

impl OIDC {
    pub fn new(
        keycloak_base_url: &str,
        client_id: String,
        client_secret: String,
        auth_callback_path: String,
        redis: Redis,
    ) -> Result<Self> {
        let base_url = Url::parse(keycloak_base_url)?;
        Ok(Self {
            auth_url: base_url.join("protocol/openid-connect/auth")?,
            token_url: base_url.join("protocol/openid-connect/token")?,
            userinfo_url: base_url.join("protocol/openid-connect/userinfo")?,
            client_id,
            client_secret,
            auth_callback_path,
            redis,
        })
    }

    pub fn get_callback_url(&self, url: &Url) -> Result<Url> {
        Ok(url.join(&self.auth_callback_path)?)
    }

    pub fn create_login_url(&self, original_url: &Url, callback_url: &Url) -> Result<Url> {
        Ok(Url::parse_with_params(
            self.auth_url.as_str(),
            &[
                ("client_id", self.client_id.as_str()),
                ("redirect_uri", callback_url.as_str()),
                ("response_type", "code"),
                ("scope", "openid"),
                ("state", original_url.as_str()),
            ],
        )?)
    }

    pub async fn get_token(&self, auth: &AuthType) -> Result<TokenResponse> {
        let mut form = vec![
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
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
            .post(self.token_url.as_str())
            .form(&form)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    pub async fn get_userinfo(&self, access_token: &str) -> Result<UserInfo> {
        Ok(Client::new()
            .get(self.userinfo_url.as_str())
            .header("Authorization", format!("Bearer {access_token}"))
            .send()
            .await?
            .json()
            .await?)
    }

    pub async fn create_session(&self, auth: CodeAuth) -> Result<Session> {
        let token = self
            .get_token(&AuthType::Code(auth))
            .await
            .wrap_err("could not fetch access token")?;
        let userinfo = self
            .get_userinfo(&token.access_token)
            .await
            .wrap_err("could not fetch user info")?;
        let session_id: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        self.redis
            .set_token(session_id.as_str(), &token)
            .await
            .wrap_err("could not store token in redis")?;
        Ok(Session {
            session_id,
            userinfo,
        })
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Session> {
        let token = self
            .redis
            .get_token(session_id)
            .await
            .wrap_err("could not fetch token from redis")?;
        let userinfo = match self.get_userinfo(&token.access_token).await {
            Ok(userinfo) => userinfo,
            Err(err) => {
                debug!("could not use access token to fetch userinfo: {:?}", err);
                let token_response = self
                    .get_token(&AuthType::RefreshToken(token.refresh_token))
                    .await
                    .wrap_err("could not refresh access token")?;
                let userinfo = self
                    .get_userinfo(&token_response.access_token)
                    .await
                    .wrap_err("could not use fresh access token to fetch userinfo")?;
                self.redis
                    .set_token(session_id, &token_response)
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

    pub async fn is_authorized(&self, session_id: &str, role: &str) -> Result<bool> {
        Ok(
            match self
                .redis
                .get_session_cache(session_id, role)
                .await
                .wrap_err("could not get session cache from redis")?
            {
                SessionCache::Allowed => true,
                SessionCache::Forbidden => false,
                SessionCache::NotCached => {
                    let result: bool = self
                        .get_session(session_id)
                        .await
                        .wrap_err("could not fetch session data")?
                        .userinfo
                        .roles
                        .contains(&role.into());
                    self.redis
                        .update_session_cache(
                            session_id,
                            role,
                            if result {
                                &SessionCache::Allowed
                            } else {
                                &SessionCache::Forbidden
                            },
                        )
                        .await
                        .wrap_err("could not update redis session cache")?;
                    result
                }
            },
        )
    }
}
