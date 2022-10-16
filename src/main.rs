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
#![allow(clippy::unused_async, clippy::module_name_repetitions)]

use std::net::SocketAddr;

use axum::Server;

mod config;
mod endpoints;
mod oidc;
mod redis;

#[allow(clippy::no_effect_underscore_binding)]
#[tokio::main]
async fn main() -> eyre::Result<()> {
    // initialize logger
    pretty_env_logger::init();

    // initialize panic and error report handler
    color_eyre::install()?;

    // load config from environment variables
    let conf = config::config();

    // start axum server
    Server::bind(&SocketAddr::new(conf.host.parse()?, conf.port))
        .serve(endpoints::router().into_make_service())
        .await?;

    Ok(())
}
