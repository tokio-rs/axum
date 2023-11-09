use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum_core::response::{IntoResponse, IntoResponseParts, Response, ResponseParts};
use http::header::LOCATION;
use http::{HeaderValue, Request};
use pin_project_lite::pin_project;
use tower_layer::Layer;
use tower_service::Service;

use crate::extract::NestedPath;

/// Middleware for fixing redirects from nested service to include the path they're nested at.
///
/// # Example
///
/// ```
/// use axum::{
///     Router,
///     routing::get,
///     middleware::FixNestedRedirectLayer,
///     response::Redirect,
/// };
///
/// let api = Router::new()
///     // redirect from `/old` to `/new`
///     .route("/old", get(|| async { Redirect::to("/new") }))
///     .route("/new", get(|| async { /* ... */ }));
///
/// let app = Router::new()
///     .nest(
///         "/api",
///         // make sure the redirects include `/api`, i.e. `location: /api/new`
///         api.layer(FixNestedRedirectLayer::default()),
///     );
/// # let _: Router = app;
/// ```
///
/// # Multiple levels of nesting
///
/// If you're nesting multiple levels of routers make sure to add `FixNestedRedirectLayer` at the
/// inner most level:
///
/// ```
/// use axum::{
///     Router,
///     routing::get,
///     middleware::FixNestedRedirectLayer,
///     response::Redirect,
/// };
///
/// let users_api = Router::new()
///     // redirect from `/old` to `/new`
///     .route("/old", get(|| async { Redirect::to("/new") }))
///     .route("/new", get(|| async { /* ... */ }));
///
/// let api = Router::new()
///     .nest(
///         "/users",
///         // add the middleware at the inner most level
///         users_api.layer(FixNestedRedirectLayer::default()),
///     );
///
/// let app = Router::new()
///     // don't add the middleware here
///     .nest("/api", api);
/// # let _: Router = app;
/// ```
///
/// # Opt-out
///
/// Individual handlers can opt-out by including `FixNestedRedirectOptOut` in the response:
///
/// ```
/// use axum::{
///     Router,
///     routing::get,
///     middleware::{FixNestedRedirectLayer, FixNestedRedirectOptOut},
///     response::Redirect,
/// };
///
/// let api = Router::new()
///     .route("/foo", get(|| async {
///         // this redirect will go to `/somewhere` and not `/api/somewhere`
///         (FixNestedRedirectOptOut, Redirect::to("/somewhere"))
///     }));
///
/// let app = Router::new()
///     .nest(
///         "/api",
///         api.layer(FixNestedRedirectLayer::default()),
///     );
/// # let _: Router = app;
/// ```
///
/// # Using with `ServeDir`
///
/// `FixNestedRedirectLayer` can also be used with tower-http's [`ServeDir`]:
///
/// ```
/// use axum::{
///     Router,
///     middleware::FixNestedRedirect,
/// };
/// use tower_http::services::ServeDir;
///
/// let app = Router::new().nest_service(
///     "/assets",
///     FixNestedRedirect::new(ServeDir::new("/assets")),
/// );
/// # let _: Router = app;
/// ```
///
/// [`ServeDir`]: tower_http::services::ServeDir
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct FixNestedRedirectLayer;

impl<S> Layer<S> for FixNestedRedirectLayer {
    type Service = FixNestedRedirect<S>;

    fn layer(&self, inner: S) -> Self::Service {
        FixNestedRedirect::new(inner)
    }
}

/// Service for fixing redirects from nested services.
///
/// See [`FixNestedRedirectLayer`] for more details.
#[derive(Clone, Debug)]
pub struct FixNestedRedirect<S> {
    inner: S,
}

impl<S> FixNestedRedirect<S> {
    /// Create a new `FixNestedRedirect`.
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for FixNestedRedirect<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let (mut parts, body) = req.into_parts();
        let nested_path = NestedPath::extract(&mut parts).ok();
        let req = Request::from_parts(parts, body);
        ResponseFuture {
            future: self.inner.call(req),
            nested_path,
        }
    }
}

pin_project! {
    /// Response future for [`FixNestedRedirect`].
    ///
    /// See [`FixNestedRedirectLayer`] for more details.
    pub struct ResponseFuture<F> {
        #[pin]
        future: F,
        nested_path: Option<NestedPath>,
    }
}

impl<F, B, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response<B>, E>>,
{
    type Output = Result<Response<B>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match futures_util::ready!(this.future.poll(cx)) {
            Ok(res) => {
                let (mut parts, body) = res.into_parts();
                if parts.extensions.get::<FixNestedRedirectOptOut>().is_none() {
                    fix_nested_redirect(&mut parts, this.nested_path.take());
                }
                let res = Response::from_parts(parts, body);
                Poll::Ready(Ok(res))
            }
            Err(err) => Poll::Ready(Err(err)),
        }
    }
}

fn fix_nested_redirect(
    parts: &mut http::response::Parts,
    nested_path: Option<NestedPath>,
) -> Option<()> {
    if !parts.status.is_redirection() {
        return Some(());
    }

    let location = parts.headers.get(LOCATION)?.to_str().ok()?;

    // not sure if there is a more robust way to detect an absolute uri 🤔
    if location.starts_with("https://")
        || location.starts_with("http://")
        || location.starts_with("//")
    {
        return Some(());
    }

    let nested_path = nested_path?;

    let new_location = format!("{}{}", nested_path.as_str().trim_end_matches('/'), location);
    let new_location = HeaderValue::from_str(&new_location).ok()?;
    parts.headers.insert(LOCATION, new_location);

    Some(())
}

/// Response extension used to opt-out of [`FixNestedRedirectLayer`] changing the `Location`
/// header.
///
/// See [`FixNestedRedirectLayer`] for more details.
#[derive(Copy, Clone, Debug)]
pub struct FixNestedRedirectOptOut;

impl IntoResponseParts for FixNestedRedirectOptOut {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        res.extensions_mut().insert(self);
        Ok(res)
    }
}

impl IntoResponse for FixNestedRedirectOptOut {
    fn into_response(self) -> Response {
        (self, ()).into_response()
    }
}

#[cfg(test)]
mod tests {
    use http::StatusCode;
    use tower_http::services::ServeDir;

    use crate::{
        middleware::{FixNestedRedirect, FixNestedRedirectLayer, FixNestedRedirectOptOut},
        response::Redirect,
        routing::get,
        test_helpers::TestClient,
        Router,
    };

    #[crate::test]
    async fn one_level() {
        let api = Router::new().route("/old", get(|| async { Redirect::to("/new") }));
        let app = Router::new().nest("/api", api.layer(FixNestedRedirectLayer));

        let client = TestClient::new(app);

        let res = client.get("/api/old").send().await;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        assert_eq!(res.headers()["location"], "/api/new");
    }

    #[crate::test]
    async fn one_level_with_trailing_slash() {
        let api = Router::new().route("/old", get(|| async { Redirect::to("/new") }));
        let app = Router::new().nest("/api/", api.layer(FixNestedRedirectLayer));

        let client = TestClient::new(app);

        let res = client.get("/api/old").send().await;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        assert_eq!(res.headers()["location"], "/api/new");
    }

    #[crate::test]
    async fn two_levels() {
        let users = Router::new().route("/old", get(|| async { Redirect::to("/new") }));
        let api = Router::new().nest("/users", users.layer(FixNestedRedirectLayer));
        let app = Router::new().nest("/api", api);

        let client = TestClient::new(app);

        let res = client.get("/api/users/old").send().await;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        assert_eq!(res.headers()["location"], "/api/users/new");
    }

    #[crate::test]
    async fn opt_out() {
        let api = Router::new().route(
            "/old",
            get(|| async {
                (
                    FixNestedRedirectOptOut,
                    Redirect::to("/other/non/api/route"),
                )
            }),
        );
        let app = Router::new().nest("/api", api.layer(FixNestedRedirectLayer));

        let client = TestClient::new(app);

        let res = client.get("/api/old").send().await;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        assert_eq!(res.headers()["location"], "/other/non/api/route");
    }

    #[crate::test]
    async fn absolute_uri() {
        let api = Router::new()
            .route("/old", get(|| async { Redirect::to("http://example.com") }))
            .route("/old2", get(|| async { Redirect::to("//example.com") }));
        let app = Router::new().nest("/api", api.layer(FixNestedRedirectLayer));

        let client = TestClient::new(app);

        let res = client.get("/api/old").send().await;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        assert_eq!(res.headers()["location"], "http://example.com");

        let res = client.get("/api/old2").send().await;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        assert_eq!(res.headers()["location"], "//example.com");
    }

    #[crate::test]
    async fn using_serve_dir() {
        let app = Router::new().nest_service(
            "/public",
            FixNestedRedirect::new(ServeDir::new(std::env::var("CARGO_MANIFEST_DIR").unwrap())),
        );

        let client = TestClient::new(app);

        let res = client.get("/public/src").send().await;
        assert!(res.status().is_redirection());
        assert_eq!(res.headers()["location"], "/public/src/");
    }
}
