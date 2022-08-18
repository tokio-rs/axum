use super::{rejection::*, FromRequestMut, FromRequestOnce};
use crate::BoxError;
use async_trait::async_trait;
use bytes::Bytes;
use http::{HeaderMap, Method, Request, Uri, Version};
use std::convert::Infallible;

#[async_trait]
impl<S, B> FromRequestOnce<S, B> for Request<B>
where
    B: Send,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_once(req: Request<B>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(req)
    }
}

#[async_trait]
impl<S, B> FromRequestMut<S, B> for Method
where
    B: Send,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_mut(req: &mut Request<B>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(req.method().clone())
    }
}

#[async_trait]
impl<S, B> FromRequestMut<S, B> for Uri
where
    B: Send,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_mut(req: &mut Request<B>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(req.uri().clone())
    }
}

#[async_trait]
impl<S, B> FromRequestMut<S, B> for Version
where
    B: Send,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_mut(req: &mut Request<B>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(req.version())
    }
}

/// Clone the headers from the request.
///
/// Prefer using [`TypedHeader`] to extract only the headers you need.
///
/// [`TypedHeader`]: https://docs.rs/axum/latest/axum/extract/struct.TypedHeader.html
#[async_trait]
impl<S, B> FromRequestMut<S, B> for HeaderMap
where
    B: Send,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_mut(req: &mut Request<B>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(req.headers().clone())
    }
}

#[async_trait]
impl<S, B> FromRequestOnce<S, B> for Bytes
where
    B: http_body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
    S: Send + Sync,
{
    type Rejection = BytesRejection;

    async fn from_request_once(req: Request<B>, _: &S) -> Result<Self, Self::Rejection> {
        let body = req.into_body();

        let bytes = crate::body::to_bytes(body)
            .await
            .map_err(FailedToBufferBody::from_err)?;

        Ok(bytes)
    }
}

#[async_trait]
impl<S, B> FromRequestOnce<S, B> for String
where
    B: http_body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
    S: Send + Sync,
{
    type Rejection = StringRejection;

    async fn from_request_once(req: Request<B>, _: &S) -> Result<Self, Self::Rejection> {
        let body = req.into_body();

        let bytes = crate::body::to_bytes(body)
            .await
            .map_err(FailedToBufferBody::from_err)?
            .to_vec();

        let string = String::from_utf8(bytes).map_err(InvalidUtf8::from_err)?;

        Ok(string)
    }
}

#[async_trait]
impl<S, B> FromRequestOnce<S, B> for http::request::Parts
where
    B: Send + 'static,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_once(req: Request<B>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(req.into_parts().0)
    }
}

fn unwrap_infallible<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(err) => match err {},
    }
}
