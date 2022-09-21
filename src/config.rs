use redis::{aio::Connection, Client, RedisResult};
use std::process::exit;
use url::Url;

use log::{error, info};
use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Environment {
    issuer: String,
    client_id: String,
    client_secret: String,
    auth_callback: String,
    redis_url: String,
    session_allowed_ttl: usize,
    session_forbidden_ttl: usize,
}

pub struct Config {
    pub auth_url: Url,
    pub token_url: Url,
    pub userinfo_url: Url,
    pub client_id: String,
    pub client_secret: String,
    pub auth_callback: String,
    redis: Client,
    pub session_allowed_ttl: usize,
    pub session_forbidden_ttl: usize,
}

impl Config {
    pub async fn get_redis_connection(&self) -> RedisResult<Connection> {
        self.redis.get_async_connection().await
    }
}

static CONFIG: Lazy<Config> = Lazy::new(|| {
    info!("loading config");
    match load_config() {
        Ok(config) => config,
        Err(err) => {
            error!("Could not read environment variables: {}", err);
            exit(1);
        }
    }
});

fn load_config() -> Result<Config, String> {
    let env: Environment = envy::from_env().map_err(|err| err.to_string())?;
    let get_url = |endpoint| {
        Url::parse(format!("{}/protocol/openid-connect/{}", env.issuer, endpoint).as_str())
            .map_err(|err| err.to_string())
    };
    Ok(Config {
        auth_url: get_url("auth")?,
        token_url: get_url("token")?,
        userinfo_url: get_url("userinfo")?,
        client_id: env.client_id,
        client_secret: env.client_secret,
        auth_callback: env.auth_callback,
        redis: Client::open(env.redis_url).map_err(|err| err.to_string())?,
        session_allowed_ttl: env.session_allowed_ttl,
        session_forbidden_ttl: env.session_forbidden_ttl,
    })
}

pub fn config() -> &'static Config {
    &CONFIG
}
