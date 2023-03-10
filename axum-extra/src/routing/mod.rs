//! Additional types for defining routes.

use axum::{
    http::Request,
    response::{IntoResponse, Redirect, Response},
    routing::{any, MethodRouter},
    Router,
};
use http::{uri::PathAndQuery, StatusCode, Uri};
use std::{borrow::Cow, convert::Infallible};
use tower_service::Service;

mod resource;

#[cfg(feature = "typed-routing")]
mod typed;

pub use self::resource::Resource;

#[cfg(feature = "typed-routing")]
pub use self::typed::WithQueryParams;
#[cfg(feature = "typed-routing")]
pub use axum_macros::TypedPath;

#[cfg(feature = "typed-routing")]
pub use self::typed::{SecondElementIs, TypedPath};

/// Extension trait that adds additional methods to [`Router`].
pub trait RouterExt<S, B>: sealed::Sealed {
    /// Add a typed `GET` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_get<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath;

    /// Add a typed `DELETE` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_delete<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath;

    /// Add a typed `HEAD` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_head<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath;

    /// Add a typed `OPTIONS` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_options<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath;

    /// Add a typed `PATCH` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_patch<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath;

    /// Add a typed `POST` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_post<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath;

    /// Add a typed `PUT` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_put<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath;

    /// Add a typed `TRACE` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_trace<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath;

    /// Add another route to the router with an additional "trailing slash redirect" route.
    ///
    /// If you add a route _without_ a trailing slash, such as `/foo`, this method will also add a
    /// route for `/foo/` that redirects to `/foo`.
    ///
    /// If you add a route _with_ a trailing slash, such as `/bar/`, this method will also add a
    /// route for `/bar` that redirects to `/bar/`.
    ///
    /// This is similar to what axum 0.5.x did by default, except this explicitly adds another
    /// route, so trying to add a `/foo/` route after calling `.route_with_tsr("/foo", /* ... */)`
    /// will result in a panic due to route overlap.
    ///
    /// # Example
    ///
    /// ```
    /// use axum::{Router, routing::get};
    /// use axum_extra::routing::RouterExt;
    ///
    /// let app = Router::new()
    ///     // `/foo/` will redirect to `/foo`
    ///     .route_with_tsr("/foo", get(|| async {}))
    ///     // `/bar` will redirect to `/bar/`
    ///     .route_with_tsr("/bar/", get(|| async {}));
    /// # let _: Router = app;
    /// ```
    fn route_with_tsr(self, path: &str, method_router: MethodRouter<S, B>) -> Self
    where
        Self: Sized;

    /// Add another route to the router with an additional "trailing slash redirect" route.
    ///
    /// This works like [`RouterExt::route_with_tsr`] but accepts any [`Service`].
    fn route_service_with_tsr<T>(self, path: &str, service: T) -> Self
    where
        T: Service<Request<B>, Error = Infallible> + Clone + Send + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
        Self: Sized;
}

impl<S, B> RouterExt<S, B> for Router<S, B>
where
    B: axum::body::HttpBody + Send + 'static,
    S: Clone + Send + Sync + 'static,
{
    #[cfg(feature = "typed-routing")]
    fn typed_get<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::get(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_delete<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::delete(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_head<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::head(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_options<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::options(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_patch<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::patch(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_post<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::post(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_put<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::put(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_trace<H, T, P>(self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::trace(handler))
    }

    #[track_caller]
    fn route_with_tsr(mut self, path: &str, method_router: MethodRouter<S, B>) -> Self
    where
        Self: Sized,
    {
        validate_tsr_path(path);
        self = self.route(path, method_router);
        add_tsr_redirect_route(self, path)
    }

    #[track_caller]
    fn route_service_with_tsr<T>(mut self, path: &str, service: T) -> Self
    where
        T: Service<Request<B>, Error = Infallible> + Clone + Send + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
        Self: Sized,
    {
        validate_tsr_path(path);
        self = self.route_service(path, service);
        add_tsr_redirect_route(self, path)
    }
}

#[track_caller]
fn validate_tsr_path(path: &str) {
    if path == "/" {
        panic!("Cannot add a trailing slash redirect route for `/`")
    }
}

fn add_tsr_redirect_route<S, B>(router: Router<S, B>, path: &str) -> Router<S, B>
where
    B: axum::body::HttpBody + Send + 'static,
    S: Clone + Send + Sync + 'static,
{
    async fn redirect_handler(uri: Uri) -> Response {
        let new_uri = map_path(uri, |path| {
            path.strip_suffix('/')
                .map(Cow::Borrowed)
                .unwrap_or_else(|| Cow::Owned(format!("{path}/")))
        });

        if let Some(new_uri) = new_uri {
            Redirect::permanent(&new_uri.to_string()).into_response()
        } else {
            StatusCode::BAD_REQUEST.into_response()
        }
    }

    if let Some(path_without_trailing_slash) = path.strip_suffix('/') {
        router.route(path_without_trailing_slash, any(redirect_handler))
    } else {
        router.route(&format!("{path}/"), any(redirect_handler))
    }
}

/// Map the path of a `Uri`.
///
/// Returns `None` if the `Uri` cannot be put back together with the new path.
fn map_path<F>(original_uri: Uri, f: F) -> Option<Uri>
where
    F: FnOnce(&str) -> Cow<'_, str>,
{
    let mut parts = original_uri.into_parts();
    let path_and_query = parts.path_and_query.as_ref()?;

    let new_path = f(path_and_query.path());

    let new_path_and_query = if let Some(query) = &path_and_query.query() {
        format!("{new_path}?{query}").parse::<PathAndQuery>().ok()?
    } else {
        new_path.parse::<PathAndQuery>().ok()?
    };
    parts.path_and_query = Some(new_path_and_query);

    Uri::from_parts(parts).ok()
}

mod sealed {
    pub trait Sealed {}
    impl<S, B> Sealed for axum::Router<S, B> {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{extract::Path, http::StatusCode, routing::get};

    #[tokio::test]
    async fn test_tsr() {
        let app = Router::new()
            .route_with_tsr("/foo", get(|| async {}))
            .route_with_tsr("/bar/", get(|| async {}));

        let client = TestClient::new(app);

        let res = client.get("/foo").send().await;
        assert_eq!(res.status(), StatusCode::OK);

        let res = client.get("/foo/").send().await;
        assert_eq!(res.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(res.headers()["location"], "/foo");

        let res = client.get("/bar/").send().await;
        assert_eq!(res.status(), StatusCode::OK);

        let res = client.get("/bar").send().await;
        assert_eq!(res.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(res.headers()["location"], "/bar/");
    }

    #[tokio::test]
    async fn tsr_with_params() {
        let app = Router::new()
            .route_with_tsr(
                "/a/:a",
                get(|Path(param): Path<String>| async move { param }),
            )
            .route_with_tsr(
                "/b/:b/",
                get(|Path(param): Path<String>| async move { param }),
            );

        let client = TestClient::new(app);

        let res = client.get("/a/foo").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "foo");

        let res = client.get("/a/foo/").send().await;
        assert_eq!(res.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(res.headers()["location"], "/a/foo");

        let res = client.get("/b/foo/").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "foo");

        let res = client.get("/b/foo").send().await;
        assert_eq!(res.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(res.headers()["location"], "/b/foo/");
    }

    #[tokio::test]
    async fn tsr_maintains_query_params() {
        let app = Router::new().route_with_tsr("/foo", get(|| async {}));

        let client = TestClient::new(app);

        let res = client.get("/foo/?a=a").send().await;
        assert_eq!(res.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(res.headers()["location"], "/foo?a=a");
    }

    #[test]
    #[should_panic = "Cannot add a trailing slash redirect route for `/`"]
    fn tsr_at_root() {
        let _: Router = Router::new().route_with_tsr("/", get(|| async move {}));
    }
}
