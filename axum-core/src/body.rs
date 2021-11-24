//! HTTP body utilities.

use crate::BoxError;
use bytes::Bytes;
use http_body::Body;

/// A boxed [`Body`] trait object.
///
/// This is used in axum as the response body type for applications. It's
/// necessary to unify multiple response bodies types into one.
pub type BoxBody = http_body::combinators::UnsyncBoxBody<Bytes, BoxError>;

/// Convert a [`http_body::Body`] into a [`BoxBody`].
pub fn boxed<B>(body: B) -> BoxBody
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    body.map_err(Into::into).boxed_unsync()
}
