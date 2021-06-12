//! Types and traits for generating responses.

use crate::Body;
use bytes::Bytes;
use http::{header, HeaderMap, HeaderValue, Response, StatusCode};
use serde::Serialize;
use std::{borrow::Cow, convert::Infallible};
use tower::util::Either;

/// Trait for generating responses.
///
/// Types that implement `IntoResponse` can be returned from handlers.
pub trait IntoResponse {
    /// Create a response.
    fn into_response(self) -> Response<Body>;
}

impl IntoResponse for () {
    fn into_response(self) -> Response<Body> {
        Response::new(Body::empty())
    }
}

impl IntoResponse for Infallible {
    fn into_response(self) -> Response<Body> {
        match self {}
    }
}

impl<T, K> IntoResponse for Either<T, K>
where
    T: IntoResponse,
    K: IntoResponse,
{
    fn into_response(self) -> Response<Body> {
        match self {
            Either::A(inner) => inner.into_response(),
            Either::B(inner) => inner.into_response(),
        }
    }
}

impl<T, E> IntoResponse for Result<T, E>
where
    T: IntoResponse,
    E: IntoResponse,
{
    fn into_response(self) -> Response<Body> {
        match self {
            Ok(value) => value.into_response(),
            Err(err) => err.into_response(),
        }
    }
}

impl IntoResponse for Response<Body> {
    fn into_response(self) -> Self {
        self
    }
}

impl IntoResponse for &'static str {
    #[inline]
    fn into_response(self) -> Response<Body> {
        Cow::Borrowed(self).into_response()
    }
}

impl IntoResponse for String {
    #[inline]
    fn into_response(self) -> Response<Body> {
        Cow::<'static, str>::Owned(self).into_response()
    }
}

impl IntoResponse for std::borrow::Cow<'static, str> {
    fn into_response(self) -> Response<Body> {
        let mut res = Response::new(Body::from(self));
        res.headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
        res
    }
}

impl IntoResponse for Bytes {
    fn into_response(self) -> Response<Body> {
        let mut res = Response::new(Body::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse for &'static [u8] {
    fn into_response(self) -> Response<Body> {
        let mut res = Response::new(Body::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse for Vec<u8> {
    fn into_response(self) -> Response<Body> {
        let mut res = Response::new(Body::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse for std::borrow::Cow<'static, [u8]> {
    fn into_response(self) -> Response<Body> {
        let mut res = Response::new(Body::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response<Body> {
        Response::builder()
            .status(self)
            .body(Body::empty())
            .unwrap()
    }
}

impl<T> IntoResponse for (StatusCode, T)
where
    T: Into<Body>,
{
    fn into_response(self) -> Response<Body> {
        Response::builder()
            .status(self.0)
            .body(self.1.into())
            .unwrap()
    }
}

impl<T> IntoResponse for (HeaderMap, T)
where
    T: Into<Body>,
{
    fn into_response(self) -> Response<Body> {
        let mut res = Response::new(self.1.into());
        *res.headers_mut() = self.0;
        res
    }
}

impl<T> IntoResponse for (StatusCode, HeaderMap, T)
where
    T: Into<Body>,
{
    fn into_response(self) -> Response<Body> {
        let mut res = Response::new(self.2.into());
        *res.status_mut() = self.0;
        *res.headers_mut() = self.1;
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
    T: Into<Body>,
{
    fn into_response(self) -> Response<Body> {
        let mut res = Response::new(self.0.into());
        res.headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html"));
        res
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
/// use tower_web::{body::Body, response::{Json, IntoResponse}};
/// use http::{Response, header::CONTENT_TYPE};
///
/// let json = json!({
///     "data": 42,
/// });
///
/// let response: Response<Body> = Json(json).into_response();
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
    fn into_response(self) -> Response<Body> {
        let bytes = match serde_json::to_vec(&self.0) {
            Ok(res) => res,
            Err(err) => {
                return Response::builder()
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Body::from(err.to_string()))
                    .unwrap();
            }
        };

        let mut res = Response::new(Body::from(bytes));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        res
    }
}
