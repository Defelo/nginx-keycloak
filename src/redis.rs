use eyre::{Context, Result};

use log::warn;
use redis::AsyncCommands;

use crate::{config::config, oidc};

macro_rules! con {
    () => {{
        &mut config()
            .get_redis_connection()
            .await
            .wrap_err("could not get redis connection")?
    }};
}

#[derive(Debug)]
pub struct Token {
    pub access_token: String,
    pub refresh_token: String,
}

pub async fn set_token(session_id: &str, token: &oidc::TokenResponse) -> Result<()> {
    redis::pipe()
        .set_ex(
            format!("access_token:{session_id}"),
            &token.access_token,
            token.expires_in,
        )
        .set_ex(
            format!("refresh_token:{session_id}"),
            &token.refresh_token,
            token.refresh_expires_in,
        )
        .query_async(con!())
        .await?;
    Ok(())
}

pub async fn get_token(session_id: &str) -> Result<Token> {
    let (access_token, refresh_token) = con!()
        .get(&[
            format!("access_token:{session_id}"),
            format!("refresh_token:{session_id}"),
        ])
        .await?;
    Ok(Token {
        access_token,
        refresh_token,
    })
}

#[derive(Debug)]
pub enum SessionCache {
    Allowed,
    Forbidden,
    NotCached,
}

pub async fn update_session_cache(
    session_id: &str,
    role: &str,
    state: &SessionCache,
) -> Result<()> {
    let key = format!("session:{session_id}:{role}");
    match state {
        SessionCache::Allowed => {
            con!()
                .set_ex(key, "allowed", config().session_allowed_ttl)
                .await?;
        }
        SessionCache::Forbidden => {
            con!()
                .set_ex(key, "forbidden", config().session_forbidden_ttl)
                .await?;
        }
        SessionCache::NotCached => {
            con!().del(key).await?;
        }
    }
    Ok(())
}

pub async fn get_session_cache(session_id: &str, role: &str) -> Result<SessionCache> {
    let value: Option<String> = con!().get(format!("session:{session_id}:{role}")).await?;

    Ok(match value {
        Some(ref s) if s == "allowed" => SessionCache::Allowed,
        Some(ref s) if s == "forbidden" => SessionCache::Forbidden,
        None => SessionCache::NotCached,
        Some(ref s) => {
            warn!("invalid session cache value for {session_id}: {s}");
            SessionCache::NotCached
        }
    })
}
