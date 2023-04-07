use super::{rejection::*, FromRequest, FromRequestParts, Request};
use crate::{body::Body, RequestExt};
use async_trait::async_trait;
use bytes::Bytes;
use http::{request::Parts, HeaderMap, Method, Uri, Version};
use std::convert::Infallible;

#[async_trait]
impl<S> FromRequest<S> for Request
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: Request, _: &S) -> Result<Self, Self::Rejection> {
        Ok(req)
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Method
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.method.clone())
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Uri
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.uri.clone())
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Version
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.version)
    }
}

/// Clone the headers from the request.
///
/// Prefer using [`TypedHeader`] to extract only the headers you need.
///
/// [`TypedHeader`]: https://docs.rs/axum/latest/axum/extract/struct.TypedHeader.html
#[async_trait]
impl<S> FromRequestParts<S> for HeaderMap
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.headers.clone())
    }
}

#[async_trait]
impl<S> FromRequest<S> for Bytes
where
    S: Send + Sync,
{
    type Rejection = BytesRejection;

    async fn from_request(req: Request, _: &S) -> Result<Self, Self::Rejection> {
        let bytes = match req.into_limited_body() {
            Ok(limited_body) => crate::body::to_bytes(limited_body)
                .await
                .map_err(FailedToBufferBody::from_err)?,
            Err(unlimited_body) => crate::body::to_bytes(unlimited_body)
                .await
                .map_err(FailedToBufferBody::from_err)?,
        };

        Ok(bytes)
    }
}

#[async_trait]
impl<S> FromRequest<S> for String
where
    S: Send + Sync,
{
    type Rejection = StringRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|err| match err {
                BytesRejection::FailedToBufferBody(inner) => {
                    StringRejection::FailedToBufferBody(inner)
                }
            })?;

        let string = std::str::from_utf8(&bytes)
            .map_err(InvalidUtf8::from_err)?
            .to_owned();

        Ok(string)
    }
}

#[async_trait]
impl<S> FromRequest<S> for Parts
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: Request, _: &S) -> Result<Self, Self::Rejection> {
        Ok(req.into_parts().0)
    }
}

#[async_trait]
impl<S> FromRequest<S> for Body
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: Request, _: &S) -> Result<Self, Self::Rejection> {
        Ok(req.into_body())
    }
}
