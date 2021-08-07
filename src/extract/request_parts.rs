use super::{rejection::*, take_body, FromRequest, RequestParts};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::Stream;
use http::{HeaderMap, Method, Request, Uri, Version};
use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};
use tower::BoxError;

#[async_trait]
impl<B> FromRequest<B> for Request<B>
where
    B: Send,
{
    type Rejection = RequestAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let RequestParts {
            method: _,
            uri: _,
            version,
            headers,
            extensions,
            body,
        } = req;

        let all_parts = version
            .as_ref()
            .zip(extensions.as_ref())
            .zip(body.as_ref())
            .zip(headers.as_ref());

        if all_parts.is_some() {
            Ok(req.into_request())
        } else {
            Err(RequestAlreadyExtracted)
        }
    }
}

#[async_trait]
impl<B> FromRequest<B> for Body<B>
where
    B: Send,
{
    type Rejection = BodyAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let body = take_body(req)?;
        Ok(Self(body))
    }
}

#[async_trait]
impl<B> FromRequest<B> for Method
where
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        Ok(req.method().clone())
    }
}

#[async_trait]
impl<B> FromRequest<B> for Uri
where
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        Ok(req.uri().clone())
    }
}

#[async_trait]
impl<B> FromRequest<B> for Version
where
    B: Send,
{
    type Rejection = VersionAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        req.take_version().ok_or(VersionAlreadyExtracted)
    }
}

#[async_trait]
impl<B> FromRequest<B> for HeaderMap
where
    B: Send,
{
    type Rejection = HeadersAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        req.take_headers().ok_or(HeadersAlreadyExtracted)
    }
}

/// Extractor that extracts the request body as a [`Stream`].
///
/// # Example
///
/// ```rust,no_run
/// use axum::prelude::*;
/// use futures::StreamExt;
///
/// async fn handler(mut stream: extract::BodyStream) {
///     while let Some(chunk) = stream.next().await {
///         // ...
///     }
/// }
///
/// let app = route("/users", get(handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// [`Stream`]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html
#[derive(Debug)]
pub struct BodyStream<B = crate::body::Body>(B);

impl<B> Stream for BodyStream<B>
where
    B: http_body::Body + Unpin,
{
    type Item = Result<B::Data, B::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.0).poll_data(cx)
    }
}

#[async_trait]
impl<B> FromRequest<B> for BodyStream<B>
where
    B: http_body::Body + Unpin + Send,
{
    type Rejection = BodyAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let body = take_body(req)?;
        let stream = BodyStream(body);
        Ok(stream)
    }
}

/// Extractor that extracts the request body.
///
/// # Example
///
/// ```rust,no_run
/// use axum::prelude::*;
/// use futures::StreamExt;
///
/// async fn handler(extract::Body(body): extract::Body) {
///     // ...
/// }
///
/// let app = route("/users", get(handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
#[derive(Debug, Default, Clone)]
pub struct Body<B = crate::body::Body>(pub B);

#[async_trait]
impl<B> FromRequest<B> for Bytes
where
    B: http_body::Body + Send,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    type Rejection = BytesRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let body = take_body(req)?;

        let bytes = hyper::body::to_bytes(body)
            .await
            .map_err(FailedToBufferBody::from_err)?;

        Ok(bytes)
    }
}

#[async_trait]
impl<B> FromRequest<B> for String
where
    B: http_body::Body + Send,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    type Rejection = StringRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let body = take_body(req)?;

        let bytes = hyper::body::to_bytes(body)
            .await
            .map_err(FailedToBufferBody::from_err)?
            .to_vec();

        let string = String::from_utf8(bytes).map_err(InvalidUtf8::from_err)?;

        Ok(string)
    }
}
