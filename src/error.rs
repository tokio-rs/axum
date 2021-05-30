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

    #[error("handler service returned an error")]
    Service(#[source] BoxError),

    #[error("request extension of type `{type_name}` was not set")]
    MissingExtension { type_name: &'static str },

    #[error("`Content-Length` header is missing but was required")]
    LengthRequired,

    #[error("response body was too large")]
    PayloadTooLarge,

    #[error("response failed with status {0}")]
    WithStatus(StatusCode),
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
        | Error::DeserializeQueryString(_) => make_response(StatusCode::BAD_REQUEST),

        Error::WithStatus(status) => make_response(status),

        Error::LengthRequired => make_response(StatusCode::LENGTH_REQUIRED),
        Error::PayloadTooLarge => make_response(StatusCode::PAYLOAD_TOO_LARGE),

        Error::MissingExtension { .. } | Error::SerializeResponseBody(_) => {
            make_response(StatusCode::INTERNAL_SERVER_ERROR)
        }

        Error::Service(err) => match err.downcast::<Error>() {
            Ok(err) => Err(*err),
            Err(err) => Err(Error::Service(err)),
        },

        err @ Error::ConsumeRequestBody(_) => Err(err),
        err @ Error::ResponseBody(_) => Err(err),
    }
}
