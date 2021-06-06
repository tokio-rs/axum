use crate::Body;
use bytes::Bytes;
use http::{header, HeaderMap, HeaderValue, Response, StatusCode};
use serde::Serialize;
use std::{borrow::Cow, convert::Infallible};
use tower::util::Either;

// TODO(david): can we change this to not be generic over the body and just use hyper::Body?
pub trait IntoResponse<B> {
    fn into_response(self) -> Response<B>;
}

impl IntoResponse<Body> for () {
    fn into_response(self) -> Response<Body> {
        Response::new(Body::empty())
    }
}

impl<B> IntoResponse<B> for Infallible {
    fn into_response(self) -> Response<B> {
        match self {}
    }
}

impl<T, K, B> IntoResponse<B> for Either<T, K>
where
    T: IntoResponse<B>,
    K: IntoResponse<B>,
{
    fn into_response(self) -> Response<B> {
        match self {
            Either::A(inner) => inner.into_response(),
            Either::B(inner) => inner.into_response(),
        }
    }
}

impl<B, T, E> IntoResponse<B> for Result<T, E>
where
    T: IntoResponse<B>,
    E: IntoResponse<B>,
{
    fn into_response(self) -> Response<B> {
        match self {
            Ok(value) => value.into_response(),
            Err(err) => err.into_response(),
        }
    }
}

impl<B> IntoResponse<B> for Response<B> {
    fn into_response(self) -> Self {
        self
    }
}

impl IntoResponse<Body> for &'static str {
    #[inline]
    fn into_response(self) -> Response<Body> {
        Cow::Borrowed(self).into_response()
    }
}

impl IntoResponse<Body> for String {
    #[inline]
    fn into_response(self) -> Response<Body> {
        Cow::<'static, str>::Owned(self).into_response()
    }
}

impl IntoResponse<Body> for std::borrow::Cow<'static, str> {
    fn into_response(self) -> Response<Body> {
        let mut res = Response::new(Body::from(self));
        res.headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
        res
    }
}

impl IntoResponse<Body> for Bytes {
    fn into_response(self) -> Response<Body> {
        let mut res = Response::new(Body::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse<Body> for &'static [u8] {
    fn into_response(self) -> Response<Body> {
        let mut res = Response::new(Body::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse<Body> for Vec<u8> {
    fn into_response(self) -> Response<Body> {
        let mut res = Response::new(Body::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse<Body> for std::borrow::Cow<'static, [u8]> {
    fn into_response(self) -> Response<Body> {
        let mut res = Response::new(Body::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse<Body> for StatusCode {
    fn into_response(self) -> Response<Body> {
        Response::builder()
            .status(self)
            .body(Body::empty())
            .unwrap()
    }
}

impl<T> IntoResponse<Body> for (StatusCode, T)
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

impl<T> IntoResponse<Body> for (StatusCode, HeaderMap, T)
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

pub struct Html<T>(pub T);

impl<T> IntoResponse<Body> for Html<T>
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

pub struct Json<T>(pub T);

impl<T> IntoResponse<Body> for Json<T>
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
