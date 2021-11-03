use super::{rejection::*, take_body, Extension, FromRequest, RequestParts};
use crate::{body::Body, BoxError, Error};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::Stream;
use http::{Extensions, HeaderMap, Method, Request, Uri, Version};
use http_body::Body as HttpBody;
use std::{
    convert::Infallible,
    fmt,
    pin::Pin,
    task::{Context, Poll},
};
use sync_wrapper::SyncWrapper;

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
impl<B> FromRequest<B> for RawBody<B>
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

/// Extractor that gets the original request URI regardless of nesting.
///
/// This is necessary since [`Uri`](http::Uri), when used as an extractor, will
/// have the prefix stripped if used in a nested service.
///
/// # Example
///
/// ```
/// use axum::{
///     routing::get,
///     Router,
///     extract::OriginalUri,
///     http::Uri
/// };
///
/// let api_routes = Router::new()
///     .route(
///         "/users",
///         get(|uri: Uri, OriginalUri(original_uri): OriginalUri| async {
///             // `uri` is `/users`
///             // `original_uri` is `/api/users`
///         }),
///     );
///
/// let app = Router::new().nest("/api", api_routes);
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
#[derive(Debug, Clone)]
pub struct OriginalUri(pub Uri);

#[async_trait]
impl<B> FromRequest<B> for OriginalUri
where
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let uri = Extension::<Self>::from_request(req)
            .await
            .unwrap_or_else(|_| Extension(OriginalUri(req.uri().clone())))
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
/// Note if your request body is [`body::Body`] you can extract that directly
/// and since it already implements [`Stream`] you don't need this type. The
/// purpose of this type is to extract other types of request bodies as a
/// [`Stream`].
///
/// # Example
///
/// ```rust,no_run
/// use axum::{
///     extract::BodyStream,
///     routing::get,
///     Router,
/// };
/// use futures::StreamExt;
///
/// async fn handler(mut stream: BodyStream) {
///     while let Some(chunk) = stream.next().await {
///         // ...
///     }
/// }
///
/// let app = Router::new().route("/users", get(handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// [`Stream`]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html
/// [`body::Body`]: crate::body::Body
pub struct BodyStream(
    SyncWrapper<Pin<Box<dyn http_body::Body<Data = Bytes, Error = Error> + Send + 'static>>>,
);

impl Stream for BodyStream {
    type Item = Result<Bytes, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(self.0.get_mut()).poll_data(cx)
    }
}

#[async_trait]
impl<B> FromRequest<B> for BodyStream
where
    B: HttpBody + Send + 'static,
    B::Data: Into<Bytes>,
    B::Error: Into<BoxError>,
{
    type Rejection = BodyAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let body = take_body(req)?
            .map_data(Into::into)
            .map_err(|err| Error::new(err.into()));
        let stream = BodyStream(SyncWrapper::new(Box::pin(body)));
        Ok(stream)
    }
}

impl fmt::Debug for BodyStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BodyStream").finish()
    }
}

#[test]
fn body_stream_traits() {
    crate::test_helpers::assert_send::<BodyStream>();
    crate::test_helpers::assert_sync::<BodyStream>();
}

/// Extractor that extracts the raw request body.
///
/// Note that [`body::Body`] can be extracted directly. This purpose of this
/// type is to extract other types of request bodies.
///
/// # Example
///
/// ```rust,no_run
/// use axum::{
///     extract::RawBody,
///     routing::get,
///     Router,
/// };
/// use futures::StreamExt;
///
/// async fn handler(RawBody(body): RawBody) {
///     // ...
/// }
///
/// let app = Router::new().route("/users", get(handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// [`body::Body`]: crate::body::Body
#[derive(Debug, Default, Clone)]
pub struct RawBody<B = Body>(pub B);

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
impl FromRequest<Body> for Body {
    type Rejection = BodyAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<Body>) -> Result<Self, Self::Rejection> {
        req.take_body().ok_or(BodyAlreadyExtracted)
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
    use crate::{body::Body, routing::post, test_helpers::*, Router};
    use http::StatusCode;

    #[tokio::test]
    async fn multiple_request_extractors() {
        async fn handler(_: Request<Body>, _: Request<Body>) {}

        let app = Router::new().route("/", post(handler));

        let client = TestClient::new(app);

        let res = client.post("/").body("hi there").send().await;
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            res.text().await,
            "Cannot have two request body extractors for a single handler"
        );
    }
}
