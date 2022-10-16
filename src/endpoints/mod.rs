use axum::{routing::get, Router};

mod auth;

pub fn router() -> Router {
    Router::new().route("/auth", get(auth::auth))
}
