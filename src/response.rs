//! Types and traits for generating responses.

use crate::{
    body::{box_body, BoxBody, BoxStdError},
    Body,
};
use bytes::Bytes;
use http::{header, HeaderMap, HeaderValue, Response, StatusCode};
use http_body::{Empty, Full};
use serde::Serialize;
use std::{borrow::Cow, convert::Infallible};
use tower::{util::Either, BoxError};

/// Trait for generating responses.
///
/// Types that implement `IntoResponse` can be returned from handlers.
pub trait IntoResponse {
    type Body: http_body::Body<Data = Bytes, Error = Self::BodyError> + Send + Sync + 'static;
    type BodyError: Into<BoxError>;

    /// Create a response.
    fn into_response(self) -> Response<Self::Body>;
}

impl IntoResponse for () {
    type Body = Empty<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        Response::new(Empty::new())
    }
}

impl IntoResponse for Infallible {
    type Body = Empty<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        match self {}
    }
}

impl<T, K> IntoResponse for Either<T, K>
where
    T: IntoResponse,
    K: IntoResponse,
{
    type Body = BoxBody;
    type BodyError = BoxStdError;

    fn into_response(self) -> Response<Self::Body> {
        match self {
            Either::A(inner) => inner.into_response().map(box_body),
            Either::B(inner) => inner.into_response().map(box_body),
        }
    }
}

impl<T, E> IntoResponse for Result<T, E>
where
    T: IntoResponse,
    E: IntoResponse,
{
    type Body = BoxBody;
    type BodyError = BoxStdError;

    fn into_response(self) -> Response<Self::Body> {
        match self {
            Ok(value) => value.into_response().map(box_body),
            Err(err) => err.into_response().map(box_body),
        }
    }
}

impl<B> IntoResponse for Response<B>
where
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError>,
{
    type Body = B;
    type BodyError = <B as http_body::Body>::Error;

    fn into_response(self) -> Self {
        self
    }
}

macro_rules! impl_into_response_for_body {
    ($body:ty) => {
        impl IntoResponse for $body {
            type Body = $body;
            type BodyError = <$body as http_body::Body>::Error;

            fn into_response(self) -> Response<Self> {
                Response::new(self)
            }
        }
    };
}

impl_into_response_for_body!(Body);
impl_into_response_for_body!(Full<Bytes>);
impl_into_response_for_body!(Empty<Bytes>);
impl_into_response_for_body!(BoxBody);

impl IntoResponse for &'static str {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    #[inline]
    fn into_response(self) -> Response<Self::Body> {
        Cow::Borrowed(self).into_response()
    }
}

impl IntoResponse for String {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    #[inline]
    fn into_response(self) -> Response<Self::Body> {
        Cow::<'static, str>::Owned(self).into_response()
    }
}

impl IntoResponse for std::borrow::Cow<'static, str> {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Full::from(self));
        res.headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
        res
    }
}

impl IntoResponse for Bytes {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Full::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse for &'static [u8] {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Full::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse for Vec<u8> {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Full::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse for std::borrow::Cow<'static, [u8]> {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Full::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse for StatusCode {
    type Body = Empty<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        Response::builder().status(self).body(Empty::new()).unwrap()
    }
}

impl<T> IntoResponse for (StatusCode, T)
where
    T: IntoResponse,
{
    type Body = T::Body;
    type BodyError = T::BodyError;

    fn into_response(self) -> Response<T::Body> {
        let mut res = self.1.into_response();
        *res.status_mut() = self.0;
        res
    }
}

impl<T> IntoResponse for (HeaderMap, T)
where
    T: IntoResponse,
{
    type Body = T::Body;
    type BodyError = T::BodyError;

    fn into_response(self) -> Response<T::Body> {
        let mut res = self.1.into_response();
        *res.headers_mut() = self.0;
        res
    }
}

impl<T> IntoResponse for (StatusCode, HeaderMap, T)
where
    T: IntoResponse,
{
    type Body = T::Body;
    type BodyError = T::BodyError;

    fn into_response(self) -> Response<T::Body> {
        let mut res = self.2.into_response();
        *res.status_mut() = self.0;
        *res.headers_mut() = self.1;
        res
    }
}

impl IntoResponse for HeaderMap {
    type Body = Empty<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Empty::new());
        *res.headers_mut() = self;
        res
    }
}

/// An HTML response.
///
/// Will automatically get `Content-Type: text/html`.
#[derive(Clone, Copy, Debug)]
pub struct Html<T>(pub T);

impl<T> IntoResponse for Html<T>
where
    T: Into<Full<Bytes>>,
{
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(self.0.into());
        res.headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html"));
        res
    }
}

impl<T> From<T> for Html<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

/// A JSON response.
///
/// Can be created from any type that implements [`serde::Serialize`].
///
/// Will automatically get `Content-Type: application/json`.
///
/// # Example
///
/// ```
/// use serde_json::json;
/// use axum::{body::Body, response::{Json, IntoResponse}};
/// use http::{Response, header::CONTENT_TYPE};
/// use http_body::Full;
/// use bytes::Bytes;
///
/// let json = json!({
///     "data": 42,
/// });
///
/// let response: Response<Full<Bytes>> = Json(json).into_response();
///
/// assert_eq!(
///     response.headers().get(CONTENT_TYPE).unwrap(),
///     "application/json",
/// );
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Json<T>(pub T);

impl<T> IntoResponse for Json<T>
where
    T: Serialize,
{
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let bytes = match serde_json::to_vec(&self.0) {
            Ok(res) => res,
            Err(err) => {
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Full::from(err.to_string()))
                    .unwrap();
            }
        };

        let mut res = Response::new(Full::from(bytes));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        res
    }
}

impl<T> From<T> for Json<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}
