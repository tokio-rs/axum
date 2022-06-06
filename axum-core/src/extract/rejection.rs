//! Rejection response types.

use crate::response::{IntoResponse, Response};
use http::StatusCode;
use http_body::LengthLimitError;
use std::fmt;

/// Rejection type used if you try and extract the request body more than
/// once.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BodyAlreadyExtracted;

impl BodyAlreadyExtracted {
    const BODY: &'static str = "Cannot have two request body extractors for a single handler";
}

impl IntoResponse for BodyAlreadyExtracted {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Self::BODY).into_response()
    }
}

impl fmt::Display for BodyAlreadyExtracted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Self::BODY)
    }
}

impl std::error::Error for BodyAlreadyExtracted {}

/// Rejection type for extractors that buffer the request body. Used if the
/// request body cannot be buffered due to an error.
// TODO: in next major for axum-core make this a #[non_exhaustive] enum so we don't need the
// additional indirection
#[derive(Debug)]
pub struct FailedToBufferBody(FailedToBufferBodyInner);

impl FailedToBufferBody {
    /// Check if the body failed to be buffered because a length limit was hit.
    ///
    /// This can  _only_ happen when you're using [`tower_http::limit::RequestBodyLimitLayer`] or
    /// otherwise wrapping request bodies in [`http_body::Limited`].
    ///
    /// [`tower_http::limit::RequestBodyLimitLayer`]: https://docs.rs/tower-http/latest/tower_http/limit/struct.RequestBodyLimitLayer.html
    pub fn is_length_limit_error(&self) -> bool {
        matches!(self.0, FailedToBufferBodyInner::LengthLimitError(_))
    }
}

#[derive(Debug)]
enum FailedToBufferBodyInner {
    Unknown(crate::Error),
    LengthLimitError(LengthLimitError),
}

impl FailedToBufferBody {
    pub(crate) fn from_err<E>(err: E) -> Self
    where
        E: Into<crate::BoxError>,
    {
        let err = err.into();
        match err.downcast::<LengthLimitError>() {
            Ok(err) => Self(FailedToBufferBodyInner::LengthLimitError(*err)),
            Err(err) => Self(FailedToBufferBodyInner::Unknown(crate::Error::new(err))),
        }
    }
}

impl crate::response::IntoResponse for FailedToBufferBody {
    fn into_response(self) -> crate::response::Response {
        match self.0 {
            FailedToBufferBodyInner::Unknown(err) => (
                http::StatusCode::BAD_REQUEST,
                format!(concat!("Failed to buffer the request body", ": {}"), err),
            )
                .into_response(),
            FailedToBufferBodyInner::LengthLimitError(err) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(concat!("Failed to buffer the request body", ": {}"), err),
            )
                .into_response(),
        }
    }
}

impl std::fmt::Display for FailedToBufferBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to buffer the request body")
    }
}

impl std::error::Error for FailedToBufferBody {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.0 {
            FailedToBufferBodyInner::Unknown(err) => Some(err),
            FailedToBufferBodyInner::LengthLimitError(err) => Some(err),
        }
    }
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Request body didn't contain valid UTF-8"]
    /// Rejection type used when buffering the request into a [`String`] if the
    /// body doesn't contain valid UTF-8.
    pub struct InvalidUtf8(Error);
}

composite_rejection! {
    /// Rejection used for [`Bytes`](bytes::Bytes).
    ///
    /// Contains one variant for each way the [`Bytes`](bytes::Bytes) extractor
    /// can fail.
    pub enum BytesRejection {
        BodyAlreadyExtracted,
        FailedToBufferBody,
    }
}

composite_rejection! {
    /// Rejection used for [`String`].
    ///
    /// Contains one variant for each way the [`String`] extractor can fail.
    pub enum StringRejection {
        BodyAlreadyExtracted,
        FailedToBufferBody,
        InvalidUtf8,
    }
}
