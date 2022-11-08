//! Additional types for defining routes.

use axum::{
    handler::{Handler, HandlerWithoutStateExt},
    http::Request,
    response::{IntoResponse, Redirect},
    routing::{any, MethodRouter},
    Router,
};
use std::{convert::Infallible, future::ready, sync::Arc};
use tower_service::Service;

mod resource;

#[cfg(feature = "spa")]
mod spa;

#[cfg(feature = "typed-routing")]
mod typed;

pub use self::resource::Resource;

#[cfg(feature = "typed-routing")]
pub use axum_macros::TypedPath;

#[cfg(feature = "typed-routing")]
pub use self::typed::{SecondElementIs, TypedPath};

#[cfg(feature = "spa")]
pub use self::spa::SpaRouter;

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
        H: Handler<T, S, B>,
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
        H: Handler<T, S, B>,
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
        H: Handler<T, S, B>,
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
        H: Handler<T, S, B>,
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
        H: Handler<T, S, B>,
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
        H: Handler<T, S, B>,
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
        H: Handler<T, S, B>,
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
        H: Handler<T, S, B>,
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
        H: Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::get(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_delete<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::delete(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_head<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::head(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_options<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::options(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_patch<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::patch(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_post<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::post(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_put<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, S, B>,
        T: SecondElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::put(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_trace<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, S, B>,
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
        self = self.route(path, method_router);

        let redirect_service = {
            let path: Arc<str> = path.into();
            (move || ready(Redirect::permanent(&path))).into_service()
        };

        if let Some(path_without_trailing_slash) = path.strip_suffix('/') {
            self.route_service(path_without_trailing_slash, redirect_service)
        } else {
            self.route_service(&format!("{}/", path), redirect_service)
        }
    }

    #[track_caller]
    fn route_service_with_tsr<T>(mut self, path: &str, service: T) -> Self
    where
        T: Service<Request<B>, Error = Infallible> + Clone + Send + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
        Self: Sized,
    {
        self = self.route_service(path, service);

        let redirect = Redirect::permanent(path);

        if let Some(path_without_trailing_slash) = path.strip_suffix('/') {
            self.route(
                path_without_trailing_slash,
                any(move || ready(redirect.clone())),
            )
        } else {
            self.route(&format!("{}/", path), any(move || ready(redirect.clone())))
        }
    }
}

mod sealed {
    pub trait Sealed {}
    impl<S, B> Sealed for axum::Router<S, B> {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{http::StatusCode, routing::get};

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
}
