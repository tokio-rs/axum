use super::{rejection::*, FromRequest, FromRequestParts, Request};
use crate::{body::Body, RequestExt};
use async_trait::async_trait;
use bytes::{Buf as _, BufMut, Bytes, BytesMut};
use http::{request::Parts, Extensions, HeaderMap, Method, Uri, Version};
use http_body::Body as _;
use http_body_util::BodyExt;
use std::{
    convert::Infallible,
    pin::{pin, Pin},
};

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
/// [`TypedHeader`]: https://docs.rs/axum/0.7/axum/extract/struct.TypedHeader.html
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
impl<S> FromRequest<S> for BytesMut
where
    S: Send + Sync,
{
    type Rejection = BytesRejection;

    async fn from_request(req: Request, _: &S) -> Result<Self, Self::Rejection> {
        let body = pin!(req.into_limited_body());
        let mut bytes = BytesMut::new();
        body_to_bytes_mut(body, &mut bytes).await?;
        Ok(bytes)
    }
}

async fn body_to_bytes_mut(
    mut body: Pin<&mut Body>,
    bytes: &mut BytesMut,
) -> Result<(), BytesRejection> {
    while let Some(frame) = body
        .frame()
        .await
        .transpose()
        .map_err(FailedToBufferBody::from_err)?
    {
        let Ok(data) = frame.into_data() else {
            return Ok(());
        };
        bytes.put(data);
    }

    Ok(())
}

#[async_trait]
impl<S> FromRequest<S> for Bytes
where
    S: Send + Sync,
{
    type Rejection = BytesRejection;

    async fn from_request(req: Request, _: &S) -> Result<Self, Self::Rejection> {
        let mut body = pin!(req.into_limited_body());

        // If there's only 1 chunk, we can just return Buf::to_bytes()
        let first_chunk = if let Some(frame) = body
            .frame()
            .await
            .transpose()
            .map_err(FailedToBufferBody::from_err)?
        {
            let Ok(first_chunk) = frame.into_data() else {
                return Ok(Bytes::new());
            };
            first_chunk
        } else {
            return Ok(Bytes::new());
        };

        let mut bytes = if let Some(frame) = body
            .frame()
            .await
            .transpose()
            .map_err(FailedToBufferBody::from_err)?
        {
            let Ok(second_chunk) = frame.into_data() else {
                return Ok(first_chunk);
            };

            let cap = first_chunk.remaining()
                + second_chunk.remaining()
                + body.size_hint().lower() as usize;

            let mut bytes = BytesMut::with_capacity(cap);
            bytes.put(first_chunk);
            bytes.put(second_chunk);
            bytes
        } else {
            return Ok(first_chunk);
        };

        body_to_bytes_mut(body, &mut bytes).await?;
        Ok(bytes.freeze())
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
impl<S> FromRequestParts<S> for Parts
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.clone())
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Extensions
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.extensions.clone())
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
