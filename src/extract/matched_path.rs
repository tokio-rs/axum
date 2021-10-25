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
#[derive(Clone, Debug)]
pub struct MatchedPath(pub(crate) Arc<str>);

impl MatchedPath {
    /// Returns a `str` representation of the path.
    pub fn as_str(&self) -> &str {
        &*self.0
    }
}

#[async_trait]
impl<B> FromRequest<B> for MatchedPath
where
    B: Send,
{
    type Rejection = MatchedPathRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let extensions =
            req.extensions()
                .ok_or(MatchedPathRejection::ExtensionsAlreadyExtracted(
                    ExtensionsAlreadyExtracted,
                ))?;

        let matched_path = extensions
            .get::<Self>()
            .ok_or(MatchedPathRejection::MatchedPathMissing(MatchedPathMissing))?
            .clone();

        Ok(matched_path)
    }
}
