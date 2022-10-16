use eyre::{Context, Result};
use redis::{aio::Connection, Client};
use std::process::exit;
use url::Url;

use log::{debug, error, info};
use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Environment {
    host: String,
    port: u16,
    keycloak_base_url: String,
    client_id: String,
    client_secret: String,
    auth_callback_path: String,
    redis_url: String,
    session_allowed_ttl: usize,
    session_forbidden_ttl: usize,
}

#[derive(Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
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
    pub async fn get_redis_connection(&self) -> Result<Connection> {
        Ok(self.redis.get_async_connection().await?)
    }
}

static CONFIG: Lazy<Config> = Lazy::new(|| {
    info!("loading config");
    let conf = match load_config() {
        Ok(config) => config,
        Err(err) => {
            error!("Could not read environment variables: {}", err);
            exit(1);
        }
    };
    debug!("config loaded: {:#?}", conf);
    conf
});

fn load_config() -> Result<Config> {
    let env: Environment = envy::from_env()?;
    let get_url = |endpoint| {
        Url::parse(
            format!(
                "{}/protocol/openid-connect/{}",
                env.keycloak_base_url, endpoint
            )
            .as_str(),
        )
    };
    Ok(Config {
        host: env.host,
        port: env.port,
        auth_url: get_url("auth").wrap_err("could not parse auth url")?,
        token_url: get_url("token").wrap_err("could not parse token url")?,
        userinfo_url: get_url("userinfo").wrap_err("could not parse userinfo url")?,
        client_id: env.client_id,
        client_secret: env.client_secret,
        auth_callback: env.auth_callback_path,
        redis: Client::open(env.redis_url).wrap_err("could not create redis client")?,
        session_allowed_ttl: env.session_allowed_ttl,
        session_forbidden_ttl: env.session_forbidden_ttl,
    })
}

pub fn config() -> &'static Config {
    &CONFIG
}
