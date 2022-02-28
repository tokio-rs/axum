//! Rejection response types.

use crate::response::{IntoResponse, Response};
use http::StatusCode;
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

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to buffer the request body"]
    /// Rejection type for extractors that buffer the request body. Used if the
    /// request body cannot be buffered due to an error.
    pub struct FailedToBufferBody(Error);
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
