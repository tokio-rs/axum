//! HTTP body utilities.

use crate::Error;
use tower::BoxError;

#[doc(no_inline)]
pub use http_body::{Body as HttpBody, Empty, Full};

#[doc(no_inline)]
pub use hyper::body::Body;

#[doc(no_inline)]
pub use bytes::Bytes;

/// A boxed [`Body`] trait object.
///
/// This is used in axum as the response body type for applications. Its
/// necessary to unify multiple response bodies types into one.
pub type BoxBody = http_body::combinators::BoxBody<Bytes, Error>;

/// Convert a [`http_body::Body`] into a [`BoxBody`].
pub fn box_body<B>(body: B) -> BoxBody
where
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError>,
{
    body.map_err(Error::new).boxed()
}

pub(crate) fn empty() -> BoxBody {
    box_body(http_body::Empty::new())
}
