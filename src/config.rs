use eyre::Result;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub keycloak_base_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub auth_callback_path: String,
    pub redis_url: String,
    pub session_allowed_ttl: usize,
    pub session_forbidden_ttl: usize,
}

pub fn load() -> Result<Config> {
    Ok(config::Config::builder()
        .add_source(config::Environment::default())
        .build()?
        .try_deserialize()?)
}
