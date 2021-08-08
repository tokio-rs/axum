use super::{rejection::*, take_body, Extension, FromRequest, RequestParts};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::Stream;
use http::{Extensions, HeaderMap, Method, Request, Uri, Version};
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
        let req = std::mem::replace(
            req,
            RequestParts {
                method: req.method.clone(),
                version: req.version,
                uri: req.uri.clone(),
                headers: None,
                extensions: None,
                body: None,
            },
        );

        let err = match req.try_into_request() {
            Ok(req) => return Ok(req),
            Err(err) => err,
        };

        match err.downcast::<RequestAlreadyExtracted>() {
            Ok(err) => return Err(err),
            Err(err) => unreachable!(
                "Unexpected error type from `try_into_request`: `{:?}`. This is a bug in axum, please file an issue",
                err,
            ),
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

/// Extractor that gets the request URI for a nested service.
///
/// This is necessary since [`Uri`](http::Uri), when used as an extractor, will
/// always be the full URI.
///
/// # Example
///
/// ```
/// use axum::{prelude::*, routing::nest, extract::NestedUri, http::Uri};
///
/// let api_routes = route(
///     "/users",
///     get(|uri: Uri, NestedUri(nested_uri): NestedUri| async {
///         // `uri` is `/api/users`
///         // `nested_uri` is `/users`
///     }),
/// );
///
/// let app = nest("/api", api_routes);
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
#[derive(Debug, Clone)]
pub struct NestedUri(pub Uri);

#[async_trait]
impl<B> FromRequest<B> for NestedUri
where
    B: Send,
{
    type Rejection = NotNested;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let uri = Extension::<Self>::from_request(req)
            .await
            .map_err(|_| NotNested)?
            .0;
        Ok(uri)
    }
}

#[async_trait]
impl<B> FromRequest<B> for Version
where
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        Ok(req.version())
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

#[async_trait]
impl<B> FromRequest<B> for Extensions
where
    B: Send,
{
    type Rejection = ExtensionsAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        req.take_extensions().ok_or(ExtensionsAlreadyExtracted)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{body::Body, prelude::*, tests::*};
    use http::StatusCode;

    #[tokio::test]
    async fn multiple_request_extractors() {
        async fn handler(_: Request<Body>, _: Request<Body>) {}

        let app = route("/", post(handler));

        let addr = run_in_background(app).await;

        let client = reqwest::Client::new();

        let res = client
            .post(format!("http://{}", addr))
            .body("hi there")
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            res.text().await.unwrap(),
            "Cannot have two request body extractors for a single handler"
        );
    }
}
