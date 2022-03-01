use super::{rejection::*, take_body, Extension, FromRequest, RequestParts};
use crate::{
    body::{Body, Bytes, HttpBody},
    BoxError, Error,
};
use async_trait::async_trait;
use futures_util::stream::Stream;
use http::Uri;
use std::{
    convert::Infallible,
    fmt,
    pin::Pin,
    task::{Context, Poll},
};
use sync_wrapper::SyncWrapper;

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
///
/// # Extracting via request extensions
///
/// `OriginalUri` can also be accessed from middleware via request extensions.
/// This is useful for example with [`Trace`](tower_http::trace::Trace) to
/// create a span that contains the full path, if your service might be nested:
///
/// ```
/// use axum::{
///     Router,
///     extract::OriginalUri,
///     http::Request,
///     routing::get,
/// };
/// use tower_http::trace::TraceLayer;
///
/// let api_routes = Router::new()
///     .route("/users/:id", get(|| async { /* ... */ }))
///     .layer(
///         TraceLayer::new_for_http().make_span_with(|req: &Request<_>| {
///             let path = if let Some(path) = req.extensions().get::<OriginalUri>() {
///                 // This will include `/api`
///                 path.0.path().to_owned()
///             } else {
///                 // The `OriginalUri` extension will always be present if using
///                 // `Router` unless another extractor or middleware has removed it
///                 req.uri().path().to_owned()
///             };
///             tracing::info_span!("http-request", %path)
///         }),
///     );
///
/// let app = Router::new().nest("/api", api_routes);
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
#[cfg(feature = "original-uri")]
#[derive(Debug, Clone)]
pub struct OriginalUri(pub Uri);

#[cfg(feature = "original-uri")]
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

/// Extractor that extracts the request body as a [`Stream`].
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
    SyncWrapper<Pin<Box<dyn HttpBody<Data = Bytes, Error = Error> + Send + 'static>>>,
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

#[cfg(test)]
mod tests {
    use crate::{
        body::Body,
        extract::Extension,
        routing::{get, post},
        test_helpers::*,
        Router,
    };
    use http::{Method, Request, StatusCode};

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

    #[tokio::test]
    async fn extract_request_parts() {
        #[derive(Clone)]
        struct Ext;

        async fn handler(parts: http::request::Parts) {
            assert_eq!(parts.method, Method::GET);
            assert_eq!(parts.uri, "/");
            assert_eq!(parts.version, http::Version::HTTP_11);
            assert_eq!(parts.headers["x-foo"], "123");
            parts.extensions.get::<Ext>().unwrap();
        }

        let client = TestClient::new(Router::new().route("/", get(handler)).layer(Extension(Ext)));

        let res = client.get("/").header("x-foo", "123").send().await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn extract_request_parts_doesnt_consume_the_body() {
        #[derive(Clone)]
        struct Ext;

        async fn handler(_parts: http::request::Parts, body: String) {
            assert_eq!(body, "foo");
        }

        let client = TestClient::new(Router::new().route("/", get(handler)));

        let res = client.get("/").body("foo").send().await;
        assert_eq!(res.status(), StatusCode::OK);
    }
}
