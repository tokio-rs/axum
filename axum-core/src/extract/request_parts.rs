use super::{rejection::*, FromRequest, FromRequestParts, Request};
use crate::{body::Body, RequestExt};
use bytes::Bytes;
use http::{request::Parts, Extensions, HeaderMap, Method, Uri, Version};
use http_body_util::BodyExt;
use std::convert::Infallible;

impl<S> FromRequest<S> for Request
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: Request, _: &S) -> Result<Self, Self::Rejection> {
        Ok(req)
    }
}

impl<S> FromRequestParts<S> for Method
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.method.clone())
    }
}

impl<S> FromRequestParts<S> for Uri
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.uri.clone())
    }
}

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
/// [`TypedHeader`]: https://docs.rs/axum/0.7/axum/extract/struct.TypedHeader.html
impl<S> FromRequestParts<S> for HeaderMap
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.headers.clone())
    }
}

impl<S> FromRequest<S> for Bytes
where
    S: Send + Sync,
{
    type Rejection = BytesRejection;

    async fn from_request(req: Request, _: &S) -> Result<Self, Self::Rejection> {
        let bytes = req
            .into_limited_body()
            .collect()
            .await
            .map_err(FailedToBufferBody::from_err)?
            .to_bytes();

        Ok(bytes)
    }
}

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

impl<S> FromRequestParts<S> for Parts
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.clone())
    }
}

impl<S> FromRequestParts<S> for Extensions
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.extensions.clone())
    }
}

impl<S> FromRequest<S> for Body
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: Request, _: &S) -> Result<Self, Self::Rejection> {
        Ok(req.into_body())
    }
}
