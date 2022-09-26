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

mod config;
mod endpoints;
mod oidc;
mod redis;

#[macro_use]
extern crate rocket;

#[allow(clippy::no_effect_underscore_binding)]
#[main]
async fn main() -> Result<(), rocket::Error> {
    // read environment variables from .env file
    dotenv::dotenv().ok();

    // initialize logger
    pretty_env_logger::init();

    // load config from environment variables
    config::config();

    // start rocket server
    rocket::build()
        .mount("/", routes![endpoints::auth::auth])
        .launch()
        .await
        .map(|_| ())
}
