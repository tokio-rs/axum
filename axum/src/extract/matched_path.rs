use super::{rejection::*, FromRequest, RequestParts};
use async_trait::async_trait;
use std::sync::Arc;

/// Access the path in the router that matches the request.
///
/// ```
/// use axum::{
///     Router,
///     extract::MatchedPath,
///     routing::get,
/// };
///
/// let app = Router::new().route(
///     "/users/:id",
///     get(|path: MatchedPath| async move {
///         let path = path.as_str();
///         // `path` will be "/users/:id"
///     })
/// );
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// `MatchedPath` can also be accessed from middleware via request extensions.
/// This is useful for example with [`Trace`](tower_http::trace::Trace) to
/// create a span that contains the matched path:
///
/// ```
/// use axum::{
///     Router,
///     extract::MatchedPath,
///     http::Request,
///     routing::get,
/// };
/// use tower_http::trace::TraceLayer;
///
/// let app = Router::new()
///     .route("/users/:id", get(|| async { /* ... */ }))
///     .layer(
///         TraceLayer::new_for_http().make_span_with(|req: &Request<_>| {
///             let path = if let Some(path) = req.extensions().get::<MatchedPath>() {
///                 path.as_str()
///             } else {
///                 req.uri().path()
///             };
///             tracing::info_span!("http-request", %path)
///         }),
///     );
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "matched-path")))]
#[derive(Clone, Debug)]
pub struct MatchedPath(pub(crate) Arc<str>);

impl MatchedPath {
    /// Returns a `str` representation of the path.
    pub fn as_str(&self) -> &str {
        &*self.0
    }
}

#[async_trait]
impl<S, B> FromRequest<S, B> for MatchedPath
where
    B: Send,
    S: Send,
{
    type Rejection = MatchedPathRejection;

    async fn from_request(req: &mut RequestParts<S, B>) -> Result<Self, Self::Rejection> {
        let matched_path = req
            .extensions()
            .get::<Self>()
            .ok_or(MatchedPathRejection::MatchedPathMissing(MatchedPathMissing))?
            .clone();

        Ok(matched_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        extract::Extension, handler::HandlerWithoutStateExt, routing::get, test_helpers::*, Router,
    };
    use http::{Request, StatusCode};
    use std::task::{Context, Poll};
    use tower::layer::layer_fn;
    use tower_service::Service;

    #[derive(Clone)]
    struct SetMatchedPathExtension<S>(S);

    impl<S, B> Service<Request<B>> for SetMatchedPathExtension<S>
    where
        S: Service<Request<B>>,
    {
        type Response = S::Response;
        type Error = S::Error;
        type Future = S::Future;

        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.0.poll_ready(cx)
        }

        fn call(&mut self, mut req: Request<B>) -> Self::Future {
            let path = req
                .extensions()
                .get::<MatchedPath>()
                .unwrap()
                .as_str()
                .to_owned();
            req.extensions_mut().insert(MatchedPathFromMiddleware(path));
            self.0.call(req)
        }
    }

    #[derive(Clone)]
    struct MatchedPathFromMiddleware(String);

    #[tokio::test]
    async fn access_matched_path() {
        let api = Router::new().route(
            "/users/:id",
            get(|path: MatchedPath| async move { path.as_str().to_owned() }),
        );

        async fn handler(
            path: MatchedPath,
            Extension(MatchedPathFromMiddleware(path_from_middleware)): Extension<
                MatchedPathFromMiddleware,
            >,
        ) -> String {
            format!(
                "extractor = {}, middleware = {}",
                path.as_str(),
                path_from_middleware
            )
        }

        let app = Router::new()
            .route(
                "/:key",
                get(|path: MatchedPath| async move { path.as_str().to_owned() }),
            )
            .nest("/api", api)
            .nest(
                "/public",
                Router::new()
                    .route("/assets/*path", get(handler))
                    // have to set the middleware here since otherwise the
                    // matched path is just `/public/*` since we're nesting
                    // this router
                    .layer(layer_fn(SetMatchedPathExtension)),
            )
            .nest("/foo", handler.into_service())
            .layer(layer_fn(SetMatchedPathExtension));

        let client = TestClient::new(app);

        let res = client.get("/api/users/123").send().await;
        assert_eq!(res.text().await, "/api/users/:id");

        // the router nested at `/public` doesn't handle `/`
        let res = client.get("/public").send().await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        let res = client.get("/public/assets/css/style.css").send().await;
        assert_eq!(
            res.text().await,
            "extractor = /public/assets/*path, middleware = /public/assets/*path"
        );

        let res = client.get("/foo").send().await;
        assert_eq!(res.text().await, "extractor = /foo, middleware = /foo");

        let res = client.get("/foo/").send().await;
        assert_eq!(res.text().await, "extractor = /foo/, middleware = /foo/");

        let res = client.get("/foo/bar/baz").send().await;
        assert_eq!(
            res.text().await,
            format!(
                "extractor = /foo/*{}, middleware = /foo/*{}",
                crate::routing::NEST_TAIL_PARAM,
                crate::routing::NEST_TAIL_PARAM,
            ),
        );
    }

    #[tokio::test]
    async fn nested_opaque_routers_append_to_matched_path() {
        let app = Router::new().nest(
            "/:a",
            Router::new().route(
                "/:b",
                get(|path: MatchedPath| async move { path.as_str().to_owned() }),
            ),
        );

        let client = TestClient::new(app);

        let res = client.get("/foo/bar").send().await;
        assert_eq!(res.text().await, "/:a/:b");
    }
}
