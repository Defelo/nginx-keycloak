mod config;
mod endpoints;
mod oidc;
mod redis;

use config::config;
use dotenv::dotenv;

#[macro_use]
extern crate rocket;

#[launch]
fn rocket() -> _ {
    dotenv().ok();
    pretty_env_logger::init();
    config();
    rocket::build().mount("/", routes![endpoints::auth::auth])
}
