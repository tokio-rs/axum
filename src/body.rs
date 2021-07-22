//! HTTP body utilities.

use bytes::Bytes;
use http_body::Body as _;
use std::{error::Error as StdError, fmt};
use tower::BoxError;

pub use hyper::body::Body;

/// A boxed [`Body`] trait object.
///
/// This is used in axum as the response body type for applications. Its necessary to unify
/// multiple response bodies types into one.
pub type BoxBody = http_body::combinators::BoxBody<Bytes, BoxStdError>;

pub(crate) fn box_body<B>(body: B) -> BoxBody
where
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError>,
{
    body.map_err(|err| BoxStdError(err.into())).boxed()
}

pub(crate) fn empty() -> BoxBody {
    box_body(http_body::Empty::new())
}

/// A boxed error trait object that implements [`std::error::Error`].
///
/// This is necessary for compatibility with middleware that changes the error
/// type of the response body.
#[derive(Debug)]
pub struct BoxStdError(pub(crate) tower::BoxError);

impl StdError for BoxStdError {
    fn source(&self) -> std::option::Option<&(dyn StdError + 'static)> {
        self.0.source()
    }
}

impl fmt::Display for BoxStdError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}
