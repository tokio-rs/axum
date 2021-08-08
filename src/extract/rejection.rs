//! Rejection response types.

use super::IntoResponse;
use crate::{
    body::{box_body, BoxBody},
    Error,
};
use bytes::Bytes;
use http_body::Full;
use std::convert::Infallible;
use tower::BoxError;

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "Extensions taken by other extractor"]
    /// Rejection used if the request extension has been taken by another
    /// extractor.
    pub struct ExtensionsAlreadyExtracted;
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "Headers taken by other extractor"]
    /// Rejection used if the headers has been taken by another extractor.
    pub struct HeadersAlreadyExtracted;
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to parse the request body as JSON"]
    /// Rejection type for [`Json`](super::Json).
    pub struct InvalidJsonBody(Error);
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Expected request with `Content-Type: application/json`"]
    /// Rejection type for [`Json`](super::Json) used if the `Content-Type`
    /// header is missing.
    pub struct MissingJsonContentType;
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "Missing request extension"]
    /// Rejection type for [`Extension`](super::Extension) if an expected
    /// request extension was not found.
    pub struct MissingExtension(Error);
}

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

define_rejection! {
    #[status = PAYLOAD_TOO_LARGE]
    #[body = "Request payload is too large"]
    /// Rejection type for [`ContentLengthLimit`](super::ContentLengthLimit) if
    /// the request body is too large.
    pub struct PayloadTooLarge;
}

define_rejection! {
    #[status = LENGTH_REQUIRED]
    #[body = "Content length header is required"]
    /// Rejection type for [`ContentLengthLimit`](super::ContentLengthLimit) if
    /// the request is missing the `Content-Length` header or it is invalid.
    pub struct LengthRequired;
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "No url params found for matched route. This is a bug in axum. Please open an issue"]
    /// Rejection type used if you try and extract the URL params more than once.
    pub struct MissingRouteParams;
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "Cannot have two request body extractors for a single handler"]
    /// Rejection type used if you try and extract the request body more than
    /// once.
    pub struct BodyAlreadyExtracted;
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Form requests must have `Content-Type: x-www-form-urlencoded`"]
    /// Rejection type used if you try and extract the request more than once.
    pub struct InvalidFormContentType;
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "`NestedUri` extractor used for route that isn't nested"]
    /// Rejection type used if you try and extract [`NestedUri`] from a route that
    /// isn't nested.
    ///
    /// [`NestedUri`]: crate::extract::NestedUri
    pub struct NotNested;
}

/// Rejection type for [`Path`](super::Path) if the capture route
/// param didn't have the expected type.
#[derive(Debug)]
pub struct InvalidPathParam(String);

impl InvalidPathParam {
    pub(super) fn new(err: impl Into<String>) -> Self {
        InvalidPathParam(err.into())
    }
}

impl IntoResponse for InvalidPathParam {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> http::Response<Self::Body> {
        let mut res = http::Response::new(Full::from(self.to_string()));
        *res.status_mut() = http::StatusCode::BAD_REQUEST;
        res
    }
}

impl std::fmt::Display for InvalidPathParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid URL param. {}", self.0)
    }
}

impl std::error::Error for InvalidPathParam {}

/// Rejection type for extractors that deserialize query strings if the input
/// couldn't be deserialized into the target type.
#[derive(Debug)]
pub struct FailedToDeserializeQueryString {
    error: Error,
    type_name: &'static str,
}

impl FailedToDeserializeQueryString {
    pub(super) fn new<T, E>(error: E) -> Self
    where
        E: Into<BoxError>,
    {
        FailedToDeserializeQueryString {
            error: Error::new(error),
            type_name: std::any::type_name::<T>(),
        }
    }
}

impl IntoResponse for FailedToDeserializeQueryString {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> http::Response<Self::Body> {
        let mut res = http::Response::new(Full::from(self.to_string()));
        *res.status_mut() = http::StatusCode::BAD_REQUEST;
        res
    }
}

impl std::fmt::Display for FailedToDeserializeQueryString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Failed to deserialize query string. Expected something of type `{}`. Error: {}",
            self.type_name, self.error,
        )
    }
}

impl std::error::Error for FailedToDeserializeQueryString {}

composite_rejection! {
    /// Rejection used for [`Query`](super::Query).
    ///
    /// Contains one variant for each way the [`Query`](super::Query) extractor
    /// can fail.
    pub enum QueryRejection {
        FailedToDeserializeQueryString,
    }
}

composite_rejection! {
    /// Rejection used for [`Form`](super::Form).
    ///
    /// Contains one variant for each way the [`Form`](super::Form) extractor
    /// can fail.
    pub enum FormRejection {
        InvalidFormContentType,
        FailedToDeserializeQueryString,
        FailedToBufferBody,
        BodyAlreadyExtracted,
        HeadersAlreadyExtracted,
    }
}

composite_rejection! {
    /// Rejection used for [`Json`](super::Json).
    ///
    /// Contains one variant for each way the [`Json`](super::Json) extractor
    /// can fail.
    pub enum JsonRejection {
        InvalidJsonBody,
        MissingJsonContentType,
        BodyAlreadyExtracted,
        HeadersAlreadyExtracted,
    }
}

composite_rejection! {
    /// Rejection used for [`Extension`](super::Extension).
    ///
    /// Contains one variant for each way the [`Extension`](super::Extension) extractor
    /// can fail.
    pub enum ExtensionRejection {
        MissingExtension,
        ExtensionsAlreadyExtracted,
    }
}

composite_rejection! {
    /// Rejection used for [`Path`](super::Path).
    ///
    /// Contains one variant for each way the [`Path`](super::Path) extractor
    /// can fail.
    pub enum PathParamsRejection {
        InvalidPathParam,
        MissingRouteParams,
    }
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

composite_rejection! {
    /// Rejection used for [`Request<_>`].
    ///
    /// Contains one variant for each way the [`Request<_>`] extractor can fail.
    ///
    /// [`Request<_>`]: http::Request
    pub enum RequestAlreadyExtracted {
        BodyAlreadyExtracted,
        HeadersAlreadyExtracted,
        ExtensionsAlreadyExtracted,
    }
}

/// Rejection used for [`ContentLengthLimit`](super::ContentLengthLimit).
///
/// Contains one variant for each way the
/// [`ContentLengthLimit`](super::ContentLengthLimit) extractor can fail.
#[derive(Debug)]
#[non_exhaustive]
pub enum ContentLengthLimitRejection<T> {
    #[allow(missing_docs)]
    PayloadTooLarge(PayloadTooLarge),
    #[allow(missing_docs)]
    LengthRequired(LengthRequired),
    #[allow(missing_docs)]
    HeadersAlreadyExtracted(HeadersAlreadyExtracted),
    #[allow(missing_docs)]
    Inner(T),
}

impl<T> IntoResponse for ContentLengthLimitRejection<T>
where
    T: IntoResponse,
{
    type Body = BoxBody;
    type BodyError = Error;

    fn into_response(self) -> http::Response<Self::Body> {
        match self {
            Self::PayloadTooLarge(inner) => inner.into_response().map(box_body),
            Self::LengthRequired(inner) => inner.into_response().map(box_body),
            Self::HeadersAlreadyExtracted(inner) => inner.into_response().map(box_body),
            Self::Inner(inner) => inner.into_response().map(box_body),
        }
    }
}

impl<T> std::fmt::Display for ContentLengthLimitRejection<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PayloadTooLarge(inner) => inner.fmt(f),
            Self::LengthRequired(inner) => inner.fmt(f),
            Self::HeadersAlreadyExtracted(inner) => inner.fmt(f),
            Self::Inner(inner) => inner.fmt(f),
        }
    }
}

impl<T> std::error::Error for ContentLengthLimitRejection<T>
where
    T: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PayloadTooLarge(inner) => Some(inner),
            Self::LengthRequired(inner) => Some(inner),
            Self::HeadersAlreadyExtracted(inner) => Some(inner),
            Self::Inner(inner) => Some(inner),
        }
    }
}

/// Rejection used for [`TypedHeader`](super::TypedHeader).
#[cfg(feature = "headers")]
#[cfg_attr(docsrs, doc(cfg(feature = "headers")))]
#[derive(Debug)]
pub struct TypedHeaderRejection {
    pub(super) name: &'static http::header::HeaderName,
    pub(super) err: headers::Error,
}

#[cfg(feature = "headers")]
#[cfg_attr(docsrs, doc(cfg(feature = "headers")))]
impl IntoResponse for TypedHeaderRejection {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> http::Response<Self::Body> {
        let mut res = self.to_string().into_response();
        *res.status_mut() = http::StatusCode::BAD_REQUEST;
        res
    }
}

#[cfg(feature = "headers")]
#[cfg_attr(docsrs, doc(cfg(feature = "headers")))]
impl std::fmt::Display for TypedHeaderRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.err, self.name)
    }
}

#[cfg(feature = "headers")]
#[cfg_attr(docsrs, doc(cfg(feature = "headers")))]
impl std::error::Error for TypedHeaderRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.err)
    }
}
