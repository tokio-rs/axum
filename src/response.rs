use crate::{Body, Error};
use bytes::Bytes;
use http::{header, HeaderValue, Response};
use serde::Serialize;

pub trait IntoResponse<B> {
    fn into_response(self) -> Result<Response<B>, Error>;
}

impl<B> IntoResponse<B> for Response<B> {
    fn into_response(self) -> Result<Response<B>, Error> {
        Ok(self)
    }
}

impl IntoResponse<Body> for &'static str {
    fn into_response(self) -> Result<Response<Body>, Error> {
        Ok(Response::new(Body::from(self)))
    }
}

impl IntoResponse<Body> for String {
    fn into_response(self) -> Result<Response<Body>, Error> {
        Ok(Response::new(Body::from(self)))
    }
}

impl IntoResponse<Body> for std::borrow::Cow<'static, str> {
    fn into_response(self) -> Result<Response<Body>, Error> {
        Ok(Response::new(Body::from(self)))
    }
}

impl IntoResponse<Body> for Bytes {
    fn into_response(self) -> Result<Response<Body>, Error> {
        let mut res = Response::new(Body::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        Ok(res)
    }
}

impl IntoResponse<Body> for &'static [u8] {
    fn into_response(self) -> Result<Response<Body>, Error> {
        let mut res = Response::new(Body::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        Ok(res)
    }
}

impl IntoResponse<Body> for Vec<u8> {
    fn into_response(self) -> Result<Response<Body>, Error> {
        let mut res = Response::new(Body::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        Ok(res)
    }
}

impl IntoResponse<Body> for std::borrow::Cow<'static, [u8]> {
    fn into_response(self) -> Result<Response<Body>, Error> {
        let mut res = Response::new(Body::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        Ok(res)
    }
}

pub struct Json<T>(pub T);

impl<T> IntoResponse<Body> for Json<T>
where
    T: Serialize,
{
    fn into_response(self) -> Result<Response<Body>, Error> {
        let bytes = serde_json::to_vec(&self.0).map_err(Error::SerializeResponseBody)?;
        let mut res = Response::new(Body::from(bytes));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        Ok(res)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Empty;

impl IntoResponse<Body> for Empty {
    fn into_response(self) -> Result<Response<Body>, Error> {
        Ok(Response::new(Body::empty()))
    }
}
