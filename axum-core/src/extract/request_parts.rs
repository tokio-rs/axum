use super::{
    default_body_limit::DefaultBodyLimitDisabled, rejection::*, FromRequest, FromRequestParts,
};
use crate::BoxError;
use async_trait::async_trait;
use bytes::Bytes;
use http::{request::Parts, HeaderMap, Method, Request, Uri, Version};
use std::convert::Infallible;

#[async_trait]
impl<S, B> FromRequest<S, B> for Request<B>
where
    B: Send,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: Request<B>, _: &S) -> Result<Self, Self::Rejection> {
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
impl<S, B> FromRequest<S, B> for Bytes
where
    B: http_body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
    S: Send + Sync,
{
    type Rejection = BytesRejection;

    async fn from_request(req: Request<B>, _: &S) -> Result<Self, Self::Rejection> {
        const DEFAULT_LIMIT: usize = 2_097_152; // 2 mb

        let bytes = if req.extensions().get::<DefaultBodyLimitDisabled>().is_some() {
            crate::body::to_bytes(req.into_body())
                .await
                .map_err(FailedToBufferBody::from_err)?
        } else {
            let body = http_body::Limited::new(req.into_body(), DEFAULT_LIMIT);
            crate::body::to_bytes(body)
                .await
                .map_err(FailedToBufferBody::from_err)?
        };

        Ok(bytes)
    }
}

#[async_trait]
impl<S, B> FromRequest<S, B> for String
where
    B: http_body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
    S: Send + Sync,
{
    type Rejection = StringRejection;

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
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
impl<S, B> FromRequest<S, B> for Parts
where
    B: Send + 'static,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: Request<B>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(req.into_parts().0)
    }
}
