#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::dbg_macro, clippy::use_debug)]
#![warn(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![warn(
    clippy::self_named_module_files,
    clippy::shadow_unrelated,
    clippy::str_to_string,
    clippy::wildcard_enum_match_arm
)]
#![allow(clippy::module_name_repetitions, clippy::upper_case_acronyms)]

use std::net::SocketAddr;

use axum::Server;
use log::{debug, info};
use oidc::OIDC;

mod config;
mod endpoints;
mod oidc;
mod redis;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // initialize logger
    pretty_env_logger::init();

    // initialize panic and error report handler
    color_eyre::install()?;

    // load config from environment variables
    info!("loading config");
    let config = config::load()?;
    debug!("config loaded: {config:#?}");

    // create redis client
    let redis = redis::Redis::new(
        &config.redis_url,
        config.session_allowed_ttl,
        config.session_forbidden_ttl,
    )?;

    // create oidc client
    let oidc = OIDC::new(
        &config.keycloak_base_url,
        config.client_id,
        config.client_secret,
        config.auth_callback_path,
        redis,
    )?;

    // start axum server
    info!("starting server on {}:{}", config.host, config.port);
    Server::bind(&SocketAddr::new(config.host.parse()?, config.port))
        .serve(endpoints::router(oidc).into_make_service())
        .await?;

    Ok(())
}
