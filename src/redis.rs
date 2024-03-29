use eyre::Result;
use log::warn;
use redis::{aio::Connection, AsyncCommands, Client};

use crate::oidc;

pub struct Redis {
    client: Client,
    session_allowed_ttl: u64,
    session_forbidden_ttl: u64,
}

impl Redis {
    pub fn new(
        redis_url: &str,
        session_allowed_ttl: u64,
        session_forbidden_ttl: u64,
    ) -> Result<Self> {
        Ok(Self {
            client: Client::open(redis_url)?,
            session_allowed_ttl,
            session_forbidden_ttl,
        })
    }

    async fn get_connection(&self) -> Result<Connection> {
        Ok(self.client.get_async_connection().await?)
    }

    pub async fn set_token(&self, session_id: &str, token: &oidc::TokenResponse) -> Result<()> {
        let mut con = self.get_connection().await?;
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

    pub async fn get_token(&self, session_id: &str) -> Result<Token> {
        let mut con = self.get_connection().await?;
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

    pub async fn update_session_cache(
        &self,
        session_id: &str,
        role: &str,
        state: &SessionCache,
    ) -> Result<()> {
        let mut con = self.get_connection().await?;
        let key = format!("session:{session_id}:{role}");
        match state {
            SessionCache::Allowed => {
                con.set_ex(key, "allowed", self.session_allowed_ttl).await?;
            }
            SessionCache::Forbidden => {
                con.set_ex(key, "forbidden", self.session_forbidden_ttl)
                    .await?;
            }
            SessionCache::NotCached => {
                con.del(key).await?;
            }
        }
        Ok(())
    }

    pub async fn get_session_cache(&self, session_id: &str, role: &str) -> Result<SessionCache> {
        let mut con = self.get_connection().await?;
        let value: Option<String> = con.get(format!("session:{session_id}:{role}")).await?;

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
}

#[derive(Debug)]
pub struct Token {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug)]
pub enum SessionCache {
    Allowed,
    Forbidden,
    NotCached,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_new_err() {
        assert!(Redis::new("asdiofjasfdjoi", 1337, 42).is_err());
    }

    #[test]
    fn test_new_ok() {
        let res = Redis::new("redis://my_redis_host:6379/42", 1337, 42).unwrap();
        let connection_info = res.client.get_connection_info();
        assert_eq!(
            connection_info.addr,
            redis::ConnectionAddr::Tcp("my_redis_host".to_owned(), 6379),
        );
        assert_eq!(connection_info.redis.db, 42);
        assert_eq!(connection_info.redis.username, None);
        assert_eq!(connection_info.redis.password, None);
        assert_eq!(res.session_allowed_ttl, 1337);
        assert_eq!(res.session_forbidden_ttl, 42);
    }
}
