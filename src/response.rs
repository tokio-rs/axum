//! Types and traits for generating responses.

use crate::Body;
use bytes::Bytes;
use http::{header, HeaderMap, HeaderValue, Response, StatusCode};
use std::{borrow::Cow, convert::Infallible};
use tower::util::Either;

#[doc(no_inline)]
pub use crate::Json;

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

impl IntoResponse for Body {
    fn into_response(self) -> Response<Body> {
        Response::new(self)
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
    T: IntoResponse,
{
    fn into_response(self) -> Response<Body> {
        let mut res = self.1.into_response();
        *res.status_mut() = self.0;
        res
    }
}

impl<T> IntoResponse for (HeaderMap, T)
where
    T: IntoResponse,
{
    fn into_response(self) -> Response<Body> {
        let mut res = self.1.into_response();
        res.headers_mut().extend(self.0);
        res
    }
}

impl<T> IntoResponse for (StatusCode, HeaderMap, T)
where
    T: IntoResponse,
{
    fn into_response(self) -> Response<Body> {
        let mut res = self.2.into_response();
        *res.status_mut() = self.0;
        res.headers_mut().extend(self.1);
        res
    }
}

impl IntoResponse for HeaderMap {
    fn into_response(self) -> Response<Body> {
        let mut res = Response::new(Body::empty());
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
    T: Into<Body>,
{
    fn into_response(self) -> Response<Body> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use http::header::{HeaderMap, HeaderName};

    #[test]
    fn test_merge_headers() {
        struct MyResponse;

        impl IntoResponse for MyResponse {
            fn into_response(self) -> Response<Body> {
                let mut resp = Response::new(String::new().into());
                resp.headers_mut()
                    .insert(HeaderName::from_static("a"), HeaderValue::from_static("1"));
                resp
            }
        }

        fn check(resp: impl IntoResponse) {
            let resp = resp.into_response();
            assert_eq!(
                resp.headers().get(HeaderName::from_static("a")).unwrap(),
                &HeaderValue::from_static("1")
            );
            assert_eq!(
                resp.headers().get(HeaderName::from_static("b")).unwrap(),
                &HeaderValue::from_static("2")
            );
        }

        let headers: HeaderMap =
            std::iter::once((HeaderName::from_static("b"), HeaderValue::from_static("2")))
                .collect();

        check((headers.clone(), MyResponse));
        check((StatusCode::OK, headers, MyResponse));
    }
}
