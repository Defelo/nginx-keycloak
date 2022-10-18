use std::sync::Arc;

use axum::{routing::get, Router};

use crate::oidc::OIDC;

mod auth;

pub fn router(oidc: OIDC) -> Router<Arc<OIDC>> {
    Router::with_state(Arc::new(oidc)).route("/auth", get(auth::auth))
}
