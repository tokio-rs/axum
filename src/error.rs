use std::convert::Infallible;

use http::{Response, StatusCode};
use tower::BoxError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("failed to deserialize the request body")]
    DeserializeRequestBody(#[source] serde_json::Error),

    #[error("failed to serialize the response body")]
    SerializeResponseBody(#[source] serde_json::Error),

    #[error("failed to consume the body")]
    ConsumeRequestBody(#[source] hyper::Error),

    #[error("URI contained no query string")]
    QueryStringMissing,

    #[error("failed to deserialize query string")]
    DeserializeQueryString(#[source] serde_urlencoded::de::Error),

    #[error("failed generating the response body")]
    ResponseBody(#[source] BoxError),

    #[error("some dynamic error happened")]
    Dynamic(#[source] BoxError),

    #[error("request extension of type `{type_name}` was not set")]
    MissingExtension { type_name: &'static str },

    #[error("`Content-Length` header is missing but was required")]
    LengthRequired,

    #[error("response body was too large")]
    PayloadTooLarge,

    #[error("response failed with status {0}")]
    Status(StatusCode),

    #[error("invalid URL param. Expected something of type `{type_name}`")]
    InvalidUrlParam { type_name: &'static str },

    #[error("unknown URL param `{0}`")]
    UnknownUrlParam(String),

    #[error("response body didn't contain valid UTF-8")]
    InvalidUtf8,
}

impl From<BoxError> for Error {
    fn from(err: BoxError) -> Self {
        match err.downcast::<Error>() {
            Ok(err) => *err,
            Err(err) => Error::Dynamic(err),
        }
    }
}

impl From<Infallible> for Error {
    fn from(err: Infallible) -> Self {
        match err {}
    }
}

pub(crate) fn handle_error<B>(error: Error) -> Result<Response<B>, Error>
where
    B: Default,
{
    fn make_response<B>(status: StatusCode) -> Result<Response<B>, Error>
    where
        B: Default,
    {
        let mut res = Response::new(B::default());
        *res.status_mut() = status;
        Ok(res)
    }

    match error {
        Error::DeserializeRequestBody(_)
        | Error::QueryStringMissing
        | Error::DeserializeQueryString(_)
        | Error::InvalidUrlParam { .. }
        | Error::InvalidUtf8 => make_response(StatusCode::BAD_REQUEST),

        Error::Status(status) => make_response(status),

        Error::LengthRequired => make_response(StatusCode::LENGTH_REQUIRED),
        Error::PayloadTooLarge => make_response(StatusCode::PAYLOAD_TOO_LARGE),

        Error::MissingExtension { .. }
        | Error::SerializeResponseBody(_)
        | Error::UnknownUrlParam(_) => make_response(StatusCode::INTERNAL_SERVER_ERROR),

        Error::Dynamic(err) => match err.downcast::<Error>() {
            Ok(err) => Err(*err),
            Err(err) => Err(Error::Dynamic(err)),
        },

        err @ Error::ConsumeRequestBody(_) => Err(err),
        err @ Error::ResponseBody(_) => Err(err),
    }
}
