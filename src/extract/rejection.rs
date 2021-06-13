//! Rejection response types.

use tower::BoxError;

use super::IntoResponse;
use crate::body::Body;

macro_rules! define_rejection {
    (
        #[status = $status:ident]
        #[body = $body:expr]
        $(#[$m:meta])*
        pub struct $name:ident;
    ) => {
        $(#[$m])*
        #[derive(Debug)]
        #[non_exhaustive]
        pub struct $name;

        impl IntoResponse for $name {
            fn into_response(self) -> http::Response<Body> {
                let mut res = http::Response::new(Body::from($body));
                *res.status_mut() = http::StatusCode::$status;
                res
            }
        }
    };

    (
        #[status = $status:ident]
        #[body = $body:expr]
        $(#[$m:meta])*
        pub struct $name:ident (BoxError);
    ) => {
        $(#[$m])*
        #[derive(Debug)]
        pub struct $name(pub(super) tower::BoxError);

        impl $name {
            pub(super) fn from_err<E>(err: E) -> Self
            where
                E: Into<tower::BoxError>,
            {
                Self(err.into())
            }
        }

        impl IntoResponse for $name {
            fn into_response(self) -> http::Response<Body> {
                let mut res =
                    http::Response::new(Body::from(format!(concat!($body, ": {}"), self.0)));
                *res.status_mut() = http::StatusCode::$status;
                res
            }
        }
    };
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
    #[body = "No url params found for matched route. This is a bug in tower-web. Please open an issue"]
    /// Rejection type for [`UrlParamsMap`](super::UrlParamsMap) and
    /// [`UrlParams`](super::UrlParams) if you try and extract the URL params
    /// more than once.
    pub struct MissingRouteParams;
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "Cannot have two URL capture extractors for a single handler"]
    /// Rejection type used if you try and extract the URL params more than once.
    pub struct UrlParamsAlreadyExtracted;
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
    #[body = "Cannot have two `Request<Body>` extractors for a single handler"]
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
