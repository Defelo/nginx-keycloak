use std::sync::Arc;

use axum::{routing::get, Router};

use crate::oidc::OIDC;

mod auth;

pub fn router(oidc: OIDC) -> Router {
    Router::new()
        .route("/auth", get(auth::auth))
        .with_state(Arc::new(oidc))
}
