use redis::{AsyncCommands, RedisResult};

use crate::{config::config, oidc};

#[derive(Debug)]
pub struct Token {
    pub access_token: String,
    pub refresh_token: String,
}

pub async fn set_token(session_id: &str, token: &oidc::TokenResponse) -> RedisResult<()> {
    let mut con = config().get_redis_connection().await?;
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
        .query_async(&mut con)
        .await?;
    Ok(())
}

pub async fn get_token(session_id: &str) -> RedisResult<Token> {
    let mut con = config().get_redis_connection().await?;
    let (access_token, refresh_token) = con
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
) -> RedisResult<()> {
    let mut con = config().get_redis_connection().await?;
    let key = format!("session:{session_id}:{role}");
    match state {
        SessionCache::Allowed => {
            con.set_ex(key, "allowed", config().session_allowed_ttl)
                .await
        }
        SessionCache::Forbidden => {
            con.set_ex(key, "forbidden", config().session_forbidden_ttl)
                .await
        }
        SessionCache::NotCached => con.del(key).await,
    }
}

pub async fn get_session_cache(session_id: &str, role: &str) -> RedisResult<SessionCache> {
    let mut con = config().get_redis_connection().await?;
    let value: String = con
        .get(format!("session:{session_id}:{role}"))
        .await
        .unwrap_or_default();
    Ok(match value.as_str() {
        "allowed" => SessionCache::Allowed,
        "forbidden" => SessionCache::Forbidden,
        _ => SessionCache::NotCached,
    })
}
