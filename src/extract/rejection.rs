//! Rejection response types.

use super::IntoResponse;
use crate::body::Body;
use tower::BoxError;

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "Version taken by other extractor"]
    /// Rejection used if the HTTP version has been taken by another extractor.
    pub struct VersionAlreadyExtracted;
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "URI taken by other extractor"]
    /// Rejection used if the URI has been taken by another extractor.
    pub struct UriAlreadyExtracted;
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "Method taken by other extractor"]
    /// Rejection used if the method has been taken by another extractor.
    pub struct MethodAlreadyExtracted;
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "Extensions taken by other extractor"]
    /// Rejection used if the method has been taken by another extractor.
    pub struct ExtensionsAlreadyExtracted;
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "Headers taken by other extractor"]
    /// Rejection used if the URI has been taken by another extractor.
    pub struct HeadersAlreadyExtracted;
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Query string was invalid or missing"]
    /// Rejection type for [`Query`](super::Query).
    pub struct QueryStringMissing;
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to parse the response body as JSON"]
    /// Rejection type for [`Json`](super::Json).
    pub struct InvalidJsonBody(BoxError);
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
    pub struct MissingExtension;
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to buffer the request body"]
    /// Rejection type for extractors that buffer the request body. Used if the
    /// request body cannot be buffered due to an error.
    pub struct FailedToBufferBody(BoxError);
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Response body didn't contain valid UTF-8"]
    /// Rejection type used when buffering the request into a [`String`] if the
    /// body doesn't contain valid UTF-8.
    pub struct InvalidUtf8(BoxError);
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
    /// Rejection type for [`UrlParamsMap`](super::UrlParamsMap) and
    /// [`UrlParams`](super::UrlParams) if you try and extract the URL params
    /// more than once.
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
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "Cannot have two `Request<_>` extractors for a single handler"]
    /// Rejection type used if you try and extract the request more than once.
    pub struct RequestAlreadyExtracted;
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Form requests must have `Content-Type: x-www-form-urlencoded`"]
    /// Rejection type used if you try and extract the request more than once.
    pub struct InvalidFormContentType;
}

/// Rejection type for [`UrlParams`](super::UrlParams) if the capture route
/// param didn't have the expected type.
#[derive(Debug)]
pub struct InvalidUrlParam {
    type_name: &'static str,
}

impl InvalidUrlParam {
    pub(super) fn new<T>() -> Self {
        InvalidUrlParam {
            type_name: std::any::type_name::<T>(),
        }
    }
}

impl IntoResponse for InvalidUrlParam {
    fn into_response(self) -> http::Response<Body> {
        let mut res = http::Response::new(Body::from(format!(
            "Invalid URL param. Expected something of type `{}`",
            self.type_name
        )));
        *res.status_mut() = http::StatusCode::BAD_REQUEST;
        res
    }
}

/// Rejection type for extractors that deserialize query strings if the input
/// couldn't be deserialized into the target type.
#[derive(Debug)]
pub struct FailedToDeserializeQueryString {
    error: BoxError,
    type_name: &'static str,
}

impl FailedToDeserializeQueryString {
    pub(super) fn new<T, E>(error: E) -> Self
    where
        E: Into<BoxError>,
    {
        FailedToDeserializeQueryString {
            error: error.into(),
            type_name: std::any::type_name::<T>(),
        }
    }
}

impl IntoResponse for FailedToDeserializeQueryString {
    fn into_response(self) -> http::Response<Body> {
        let mut res = http::Response::new(Body::from(format!(
            "Failed to deserialize query string. Expected something of type `{}`. Error: {}",
            self.type_name, self.error,
        )));
        *res.status_mut() = http::StatusCode::BAD_REQUEST;
        res
    }
}

composite_rejection! {
    /// Rejection used for [`Query`](super::Query).
    ///
    /// Contains one variant for each way the [`Query`](super::Query) extractor
    /// can fail.
    pub enum QueryRejection {
        UriAlreadyExtracted,
        QueryStringMissing,
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
        QueryStringMissing,
        FailedToDeserializeQueryString,
        FailedToBufferBody,
        BodyAlreadyExtracted,
        UriAlreadyExtracted,
        HeadersAlreadyExtracted,
        MethodAlreadyExtracted,
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
    /// Rejection used for [`UrlParams`](super::UrlParams).
    ///
    /// Contains one variant for each way the [`UrlParams`](super::UrlParams) extractor
    /// can fail.
    pub enum UrlParamsRejection {
        InvalidUrlParam,
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
    fn into_response(self) -> http::Response<Body> {
        match self {
            Self::PayloadTooLarge(inner) => inner.into_response(),
            Self::LengthRequired(inner) => inner.into_response(),
            Self::HeadersAlreadyExtracted(inner) => inner.into_response(),
            Self::Inner(inner) => inner.into_response(),
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
    fn into_response(self) -> http::Response<crate::Body> {
        let mut res = format!("{} ({})", self.err, self.name).into_response();
        *res.status_mut() = http::StatusCode::BAD_REQUEST;
        res
    }
}
