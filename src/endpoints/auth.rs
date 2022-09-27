use crate::config::config;
use crate::oidc::{
    create_redirect_url, create_session, get_callback_url, is_authorized, CodeAuth, Session,
};
use eyre::{eyre, Context, Report, Result};
use log::{debug, error};
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
}

#[derive(Debug)]
pub struct CallbackRequest {
    code: String,
    request_url: Url,
    callback_url: Url,
}

impl CallbackRequest {
    fn new(code: String, request_url: Url) -> Result<Self> {
        let callback_url =
            get_callback_url(&request_url).wrap_err("could not build callback url")?;
        Ok(Self {
            code,
            request_url,
            callback_url,
        })
    }
}

#[derive(Debug)]
pub enum RequestData {
    Default(DefaultRequest),
    Callback(CallbackRequest),
}

impl RequestData {
    fn new(request: &Request) -> Result<Self> {
        let uri = request
            .headers()
            .get_one("x-request-uri")
            .ok_or(eyre!("x-request-uri header not found"))?;
        let request_uri = Url::parse(uri).wrap_err("could not parse x-request-uri")?;

        if request_uri.path() == config().auth_callback {
            let get_param = |key| {
                request_uri
                    .query_pairs()
                    .find_map(|x| if x.0 == key { Some(x.1.into()) } else { None })
            };
            let code = get_param("code").ok_or(eyre!("code param not found"))?;
            let state = get_param("state").ok_or(eyre!("state param not found"))?;
            Ok(Self::Callback(CallbackRequest::new(
                code,
                Url::parse(state.as_str()).wrap_err("could not parse state param")?,
            )?))
        } else {
            Ok(Self::Default(DefaultRequest {
                request_uri,
                session_id: request
                    .cookies()
                    .get("_keycloak_auth_session")
                    .map(|cookie| cookie.value().into()),
            }))
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RequestData {
    type Error = Report;
    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match Self::new(request) {
            Ok(data) => Outcome::Success(data),
            Err(err) => Outcome::Failure((Status::BadRequest, err)),
        }
    }
}

#[derive(Debug)]
pub struct LoginResponse {
    session_id: String,
    redirect_url: String,
}

#[derive(Debug)]
pub enum ResponseData {
    Allowed,
    Redirect(String),
    Login(LoginResponse),
    Forbidden,
    Error,
}

#[rocket::async_trait]
impl<'r> Responder<'r, 'static> for ResponseData {
    fn respond_to(self, _: &'r Request<'_>) -> response::Result<'static> {
        let mut response = Response::build();
        match self {
            Self::Allowed => {
                response.status(Status::Ok);
            }
            Self::Forbidden | Self::Error => {
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
        }
        response.ok()
    }
}

async fn handle_default(request: DefaultRequest, role: &str) -> Result<ResponseData> {
    if let Some(session_id) = request.session_id {
        match is_authorized(session_id.as_str(), role).await {
            Ok(true) => {
                return Ok(ResponseData::Allowed);
            }
            Ok(false) => {
                return Ok(ResponseData::Forbidden);
            }
            Err(err) => {
                debug!("is_authorized failed: {:?}", err);
            }
        }
    }

    Ok(ResponseData::Redirect(
        create_redirect_url(
            &request.request_uri,
            &get_callback_url(&request.request_uri).wrap_err("could not build callback url")?,
        )
        .wrap_err("could not build redirect url")?,
    ))
}

async fn handle_callback(request: CallbackRequest) -> Result<ResponseData> {
    let Session { session_id, .. } = create_session(CodeAuth {
        code: request.code,
        callback_url: request.callback_url.into(),
    })
    .await
    .wrap_err("could not create session")?;

    Ok(ResponseData::Login(LoginResponse {
        session_id,
        redirect_url: request.request_url.into(),
    }))
}

#[rocket::get("/auth?<role>")]
pub async fn auth(request: RequestData, role: &str) -> Result<ResponseData, ResponseData> {
    debug!("REQUEST: {:#?}", request);
    let response = match request {
        RequestData::Default(request) => handle_default(request, role).await,
        RequestData::Callback(request) => handle_callback(request).await,
    }
    .map_err(|err| {
        error!("auth request failed: {:?}", err);
        ResponseData::Error
    });
    debug!("RESPONSE: {:#?}", response);
    response
}
