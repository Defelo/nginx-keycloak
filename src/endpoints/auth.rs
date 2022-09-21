use crate::config::config;
use crate::oidc::{
    create_redirect_url, create_session, get_callback_url, is_authorized, CodeAuth, Session,
};
use rocket::{
    http::{Header, Status},
    request::{FromRequest, Outcome},
    response,
    response::{Responder, Response},
    Request,
};
use url::Url;

#[derive(Debug)]
pub struct DefaultRequest {
    request_uri: Url,
    session_id: Option<String>,
    role: String,
}

#[derive(Debug)]
pub struct CallbackRequest {
    code: String,
    request_url: Url,
    callback_url: Url,
}

impl CallbackRequest {
    fn new(code: String, request_url: Url) -> Option<Self> {
        let callback_url = get_callback_url(&request_url)?;
        Some(Self {
            code,
            request_url,
            callback_url,
        })
    }
}

#[derive(Debug)]
pub enum AuthRequest {
    Default(DefaultRequest),
    Callback(CallbackRequest),
}

impl AuthRequest {
    fn new(request: &Request) -> Option<Self> {
        let uri = request.headers().get_one("x-request-uri")?;
        let request_uri = Url::parse(uri).ok()?;
        if request_uri.path() == config().auth_callback {
            let get_param = |key| {
                request_uri
                    .query_pairs()
                    .find_map(|x| if x.0 == key { Some(x.1.into()) } else { None })
            };
            let code = get_param("code")?;
            let state = get_param("state")?;
            Some(Self::Callback(CallbackRequest::new(
                code,
                Url::parse(state.as_str()).ok()?,
            )?))
        } else {
            Some(Self::Default(DefaultRequest {
                request_uri,
                session_id: request
                    .cookies()
                    .get("_keycloak_auth_session")
                    .map(|cookie| cookie.value().into()),
                role: request.query_value("role")?.ok()?,
            }))
        }
    }
}

#[async_trait]
impl<'r> FromRequest<'r> for AuthRequest {
    type Error = ();
    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match AuthRequest::new(request) {
            Some(headers) => Outcome::Success(headers),
            None => Outcome::Failure((Status::BadRequest, ())),
        }
    }
}

#[derive(Debug)]
pub struct LoginResponse {
    session_id: String,
    redirect_url: String,
}

#[derive(Debug)]
pub enum AuthResponder {
    Allowed,
    Redirect(String),
    Login(LoginResponse),
    Forbidden,
    Error,
}

#[async_trait]
impl<'r> Responder<'r, 'static> for AuthResponder {
    fn respond_to(self, _: &'r Request<'_>) -> response::Result<'static> {
        let mut response = Response::build();
        match self {
            Self::Allowed => {
                response.status(Status::Ok);
            }
            Self::Forbidden => {
                response.status(Status::Forbidden);
            }
            Self::Redirect(login_url) => {
                response.status(Status::Unauthorized);
                response.header(Header::new("X-Auth-Redirect", login_url));
            }
            Self::Login(LoginResponse {
                session_id,
                redirect_url,
            }) => {
                response.status(Status::Unauthorized);
                response.header(Header::new("X-Auth-Redirect", redirect_url));
                response.header(Header::new(
                    "X-Auth-Cookie",
                    format!("_keycloak_auth_session={session_id}; Secure; HttpOnly; Path=/"),
                ));
            }
            Self::Error => {
                response.status(Status::Forbidden);
            }
        }
        response.ok()
    }
}

async fn handle_default(request: DefaultRequest) -> Result<AuthResponder, AuthResponder> {
    if let Some(session_id) = request.session_id {
        match is_authorized(session_id.as_str(), &request.role).await {
            Some(true) => {
                return Ok(AuthResponder::Allowed);
            }
            Some(false) => {
                return Ok(AuthResponder::Forbidden);
            }
            None => {}
        }
    }

    let callback_url = get_callback_url(&request.request_uri).ok_or(AuthResponder::Error)?;
    let redirect_url =
        create_redirect_url(&request.request_uri, &callback_url).ok_or(AuthResponder::Error)?;
    Ok(AuthResponder::Redirect(redirect_url))
}

async fn handle_callback(request: CallbackRequest) -> Result<AuthResponder, AuthResponder> {
    let Session { session_id, .. } = create_session(CodeAuth {
        code: request.code,
        callback_url: request.callback_url.into(),
    })
    .await
    .ok_or(AuthResponder::Error)?;

    Ok(AuthResponder::Login(LoginResponse {
        session_id,
        redirect_url: request.request_url.into(),
    }))
}

#[get("/auth")]
pub async fn auth(request: AuthRequest) -> Result<AuthResponder, AuthResponder> {
    match request {
        AuthRequest::Default(request) => handle_default(request).await,
        AuthRequest::Callback(request) => handle_callback(request).await,
    }
}
