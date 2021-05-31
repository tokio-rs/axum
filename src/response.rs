use crate::Body;
use bytes::Bytes;
use http::{header, HeaderValue, Response, StatusCode};
use serde::Serialize;
use std::convert::Infallible;
use tower::{util::Either, BoxError};

pub trait IntoResponse<B> {
    fn into_response(self) -> Response<B>;

    fn boxed(self) -> BoxIntoResponse<B>
    where
        Self: Sized + 'static,
    {
        BoxIntoResponse(self.into_response())
    }
}

impl<B> IntoResponse<B> for ()
where
    B: Default,
{
    fn into_response(self) -> Response<B> {
        Response::new(B::default())
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
    fn into_response(self) -> Response<Body> {
        Response::new(Body::from(self))
    }
}

impl IntoResponse<Body> for String {
    fn into_response(self) -> Response<Body> {
        Response::new(Body::from(self))
    }
}

impl IntoResponse<Body> for std::borrow::Cow<'static, str> {
    fn into_response(self) -> Response<Body> {
        Response::new(Body::from(self))
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

pub struct Html<T>(pub T);

impl<T> IntoResponse<Body> for Html<T>
where
    T: Into<Bytes>,
{
    fn into_response(self) -> Response<Body> {
        let bytes = self.0.into();
        let mut res = Response::new(Body::from(bytes));
        res.headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html"));
        res
    }
}

pub struct BoxIntoResponse<B>(Response<B>);

impl<B> IntoResponse<B> for BoxIntoResponse<B> {
    fn into_response(self) -> Response<B> {
        self.0
    }
}

impl IntoResponse<Body> for BoxError {
    fn into_response(self) -> Response<Body> {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(self.to_string()))
            .unwrap()
    }
}
