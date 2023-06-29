use std::{env, path::PathBuf};

use config::File;
use eyre::Result;
use log::info;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub keycloak_base_url: String,
    pub client_id: String,
    #[serde(flatten)]
    pub client_secret: ClientSecret,
    pub auth_callback_path: String,
    pub redis_url: String,
    pub session_allowed_ttl: usize,
    pub session_forbidden_ttl: usize,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(PartialEq, Eq))]
#[serde(untagged)]
pub enum ClientSecret {
    String { client_secret: String },
    File { client_secret_file: PathBuf },
}

pub fn load() -> Result<Config> {
    let path = env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".to_owned());
    info!("Loading config from {path}");
    Ok(config::Config::builder()
        .add_source(File::with_name(&path).required(false))
        .add_source(config::Environment::default())
        .build()?
        .try_deserialize()?)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_load() {
        std::env::vars().for_each(|(var, _)| std::env::remove_var(var));
        std::env::set_var("HOST", "127.0.0.1");
        std::env::set_var("PORT", "80");
        std::env::set_var("KEYCLOAK_BASE_URL", "http://id.domain.de/realms/my_realm/");
        std::env::set_var("CLIENT_ID", "my_oidc_client");
        std::env::set_var("CLIENT_SECRET", "1t6IZN9qW2Ex1ZlS0OkBeATj");
        std::env::set_var("AUTH_CALLBACK_PATH", "/_auth/callback");
        std::env::set_var("REDIS_URL", "redis://my_redis:6379/42");
        std::env::set_var("SESSION_ALLOWED_TTL", "1337");
        std::env::set_var("SESSION_FORBIDDEN_TTL", "42");
        let config = load().unwrap();
        assert_eq!(
            config,
            Config {
                host: "127.0.0.1".to_owned(),
                port: 80,
                keycloak_base_url: "http://id.domain.de/realms/my_realm/".to_owned(),
                client_id: "my_oidc_client".to_owned(),
                client_secret: ClientSecret::String {
                    client_secret: "1t6IZN9qW2Ex1ZlS0OkBeATj".to_owned()
                },
                auth_callback_path: "/_auth/callback".to_owned(),
                redis_url: "redis://my_redis:6379/42".to_owned(),
                session_allowed_ttl: 1337,
                session_forbidden_ttl: 42,
            }
        );
    }
}
