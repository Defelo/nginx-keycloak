use std::sync::Arc;

use axum::{
    extract::{Query, State},
    headers::{Cookie, HeaderMapExt},
    http::HeaderMap,
    http::StatusCode,
    response::{IntoResponse, Result},
};
use eyre::Report;
use log::{debug, error};
use serde::Deserialize;
use url::Url;

use crate::oidc::{CodeAuth, Session, OIDC};

pub async fn auth(
    State(oidc): State<Arc<OIDC>>,
    Query(AuthQuery { role }): Query<AuthQuery>,
    headers: HeaderMap,
) -> axum::response::Result<AuthResponse> {
    let request_uri = Url::parse(
        headers
            .get("x-request-uri")
            .ok_or(AuthResponse::InternalError(
                "x-request-uri header not found",
                None,
            ))?
            .to_str()
            .map_err(|err| {
                AuthResponse::InternalError("invalid x-request-uri header", Some(err.into()))
            })?,
    )
    .map_err(|err| {
        AuthResponse::InternalError(
            "could not parse url in x-request-uri header",
            Some(err.into()),
        )
    })?;

    let callback_url = oidc
        .get_callback_url(&request_uri)
        .map_err(|err| AuthResponse::InternalError("could not create callback url", Some(err)))?;
    let login_url = oidc
        .create_login_url(&request_uri, &callback_url)
        .map_err(|err| AuthResponse::InternalError("could not create login url", Some(err)))?;

    if request_uri.path() == oidc.auth_callback_path {
        CallbackRequest {
            request_uri,
            callback_url,
            login_url,
        }
        .handle(&oidc)
        .await
    } else {
        AuthRequest {
            session_id: headers.typed_get::<Cookie>().and_then(|cookies| {
                cookies
                    .get("_keycloak_auth_session")
                    .map(std::borrow::ToOwned::to_owned)
            }),
            role,
            login_url,
        }
        .handle(&oidc)
        .await
    }
}

#[derive(Deserialize)]
pub struct AuthQuery {
    role: String,
}

struct AuthRequest {
    session_id: Option<String>,
    role: String,
    login_url: Url,
}

impl AuthRequest {
    async fn handle(self, oidc: &OIDC) -> Result<AuthResponse> {
        if let Some(session_id) = self.session_id {
            match oidc
                .is_authorized(session_id.as_str(), self.role.as_str())
                .await
            {
                Ok(true) => return Ok(AuthResponse::Ok),
                Ok(false) => return Ok(AuthResponse::Forbidden),
                Err(err) => {
                    debug!("is_authorized failed: {:?}", err);
                }
            };
        }
        Ok(AuthResponse::RedirectToLogin(self.login_url))
    }
}

struct CallbackRequest {
    request_uri: Url,
    callback_url: Url,
    login_url: Url,
}

impl CallbackRequest {
    async fn handle(self, oidc: &OIDC) -> Result<AuthResponse> {
        let get_param = |key| {
            self.request_uri
                .query_pairs()
                .find(|x| x.0 == key)
                .map(|x| -> String { x.1.into() })
                .ok_or_else(|| {
                    debug!("could not find {} param", key);
                    AuthResponse::RedirectToLogin(self.login_url.clone())
                })
        };
        let code = get_param("code")?;
        let state = Url::parse(get_param("state")?.as_str()).map_err(|err| {
            debug!("could not parse state param: {:?}", err);
            AuthResponse::RedirectToLogin(self.login_url.clone())
        })?;

        let Session { session_id, .. } = oidc
            .create_session(CodeAuth {
                code,
                callback_url: self.callback_url,
            })
            .await
            .map_err(|err| {
                debug!("could not create session: {:?}", err);
                AuthResponse::RedirectToLogin(self.login_url.clone())
            })?;

        Ok(AuthResponse::StoreSession(session_id, state))
    }
}

pub enum AuthResponse {
    Ok,
    Forbidden,
    RedirectToLogin(Url),
    StoreSession(String, Url),
    InternalError(&'static str, Option<Report>),
}

impl IntoResponse for AuthResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Ok => StatusCode::OK.into_response(),
            Self::Forbidden => StatusCode::FORBIDDEN.into_response(),
            Self::RedirectToLogin(url) => (
                StatusCode::UNAUTHORIZED,
                [("X-Auth-Redirect", url.as_str())],
            )
                .into_response(),
            Self::StoreSession(session_id, redirect_url) => (
                StatusCode::UNAUTHORIZED,
                [
                    ("X-Auth-Redirect", redirect_url.as_str()),
                    (
                        "X-Auth-Cookie",
                        format!("_keycloak_auth_session={session_id}; Secure; HttpOnly; Path=/")
                            .as_str(),
                    ),
                ],
            )
                .into_response(),
            Self::InternalError(error, report) => {
                error!("{}: {:?}", error, report);
                (StatusCode::INTERNAL_SERVER_ERROR, error).into_response()
            }
        }
    }
}
