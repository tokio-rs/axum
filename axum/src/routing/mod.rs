//! Routing between [`Service`]s and handlers.

use self::{future::RouteFuture, not_found::NotFound, path_router::PathRouter};
#[cfg(feature = "tokio")]
use crate::extract::connect_info::IntoMakeServiceWithConnectInfo;
#[cfg(feature = "matched-path")]
use crate::extract::MatchedPath;
use crate::{
    body::{Body, HttpBody},
    boxed::BoxedIntoRoute,
    handler::Handler,
    util::try_downcast,
};
use axum_core::{
    extract::Request,
    response::{IntoResponse, Response},
};
use std::{
    convert::Infallible,
    fmt,
    marker::PhantomData,
    sync::Arc,
    task::{Context, Poll},
};
use tower::service_fn;
use tower_layer::{layer_fn, Layer};
use tower_service::Service;

pub mod future;
pub mod method_routing;

mod into_make_service;
mod method_filter;
mod not_found;
pub(crate) mod path_router;
mod route;
mod strip_prefix;
pub(crate) mod url_params;

#[cfg(test)]
mod tests;

pub use self::{into_make_service::IntoMakeService, method_filter::MethodFilter, route::Route};

pub use self::method_routing::{
    any, any_service, connect, connect_service, delete, delete_service, get, get_service, head,
    head_service, on, on_service, options, options_service, patch, patch_service, post,
    post_service, put, put_service, trace, trace_service, MethodRouter,
};

macro_rules! panic_on_err {
    ($expr:expr) => {
        match $expr {
            Ok(x) => x,
            Err(err) => panic!("{err}"),
        }
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RouteId(u32);

/// The router type for composing handlers and services.
///
/// `Router<S>` means a router that is _missing_ a state of type `S` to be able
/// to handle requests. Thus, only `Router<()>` (i.e. without missing state) can
/// be passed to [`serve`]. See [`Router::with_state`] for more details.
///
/// [`serve`]: crate::serve()
#[must_use]
pub struct Router<S = ()> {
    inner: Arc<RouterInner<S>>,
}

impl<S> Clone for Router<S> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

struct RouterInner<S> {
    path_router: PathRouter<S>,
    default_fallback: bool,
    catch_all_fallback: Fallback<S>,
}

impl<S> Default for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S> fmt::Debug for Router<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Router")
            .field("path_router", &self.inner.path_router)
            .field("default_fallback", &self.inner.default_fallback)
            .field("catch_all_fallback", &self.inner.catch_all_fallback)
            .finish()
    }
}

pub(crate) const NEST_TAIL_PARAM: &str = "__private__axum_nest_tail_param";
#[cfg(feature = "matched-path")]
pub(crate) const NEST_TAIL_PARAM_CAPTURE: &str = "/{*__private__axum_nest_tail_param}";
pub(crate) const FALLBACK_PARAM: &str = "__private__axum_fallback";
pub(crate) const FALLBACK_PARAM_PATH: &str = "/{*__private__axum_fallback}";

macro_rules! map_inner {
    ( $self_:ident, $inner:pat_param => $expr:expr) => {
        #[allow(redundant_semicolons)]
        {
            let $inner = $self_.into_inner();
            Router {
                inner: Arc::new($expr),
            }
        }
    };
}

macro_rules! tap_inner {
    ( $self_:ident, mut $inner:ident => { $($stmt:stmt)* } ) => {
        #[allow(redundant_semicolons)]
        {
            let mut $inner = $self_.into_inner();
            $($stmt)*;
            Router {
                inner: Arc::new($inner),
            }
        }
    };
}

impl<S> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    /// Create a new `Router`.
    ///
    /// Unless you add additional routes this will respond with `404 Not Found` to
    /// all requests.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RouterInner {
                path_router: Default::default(),
                default_fallback: true,
                catch_all_fallback: Fallback::Default(Route::new(NotFound)),
            }),
        }
    }

    fn into_inner(self) -> RouterInner<S> {
        match Arc::try_unwrap(self.inner) {
            Ok(inner) => inner,
            Err(arc) => RouterInner {
                path_router: arc.path_router.clone(),
                default_fallback: arc.default_fallback,
                catch_all_fallback: arc.catch_all_fallback.clone(),
            },
        }
    }

    /// Turn off checks for compatibility with route matching syntax from 0.7.
    ///
    /// This allows usage of paths starting with a colon `:` or an asterisk `*` which are otherwise prohibited.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::{
    ///     routing::get,
    ///     Router,
    /// };
    ///
    /// let app = Router::<()>::new()
    ///     .without_v07_checks()
    ///     .route("/:colon", get(|| async {}))
    ///     .route("/*asterisk", get(|| async {}));
    ///
    /// // Our app now accepts
    /// // - GET /:colon
    /// // - GET /*asterisk
    /// # let _: Router = app;
    /// ```
    ///
    /// Adding such routes without calling this method first will panic.
    ///
    /// ```rust,should_panic
    /// use axum::{
    ///     routing::get,
    ///     Router,
    /// };
    ///
    /// // This panics...
    /// let app = Router::<()>::new()
    ///     .route("/:colon", get(|| async {}));
    /// ```
    ///
    /// # Merging
    ///
    /// When two routers are merged, v0.7 checks are disabled for route registrations on the resulting router if both of the two routers had them also disabled.
    ///
    /// # Nesting
    ///
    /// Each router needs to have the checks explicitly disabled. Nesting a router with the checks either enabled or disabled has no effect on the outer router.
    pub fn without_v07_checks(self) -> Self {
        tap_inner!(self, mut this => {
            this.path_router.without_v07_checks();
        })
    }

    /// Add another route to the router.
    ///
    /// `path` is a string of path segments separated by `/`. Each segment
    /// can be either static, a capture, or a wildcard.
    ///
    /// `method_router` is the [`MethodRouter`] that should receive the request if the
    /// path matches `path`. Usually, `method_router` will be a handler wrapped in a method
    /// router like [`get`]. See [`handler`](crate::handler) for more details on handlers.
    ///
    /// # Static paths
    ///
    /// Examples:
    ///
    /// - `/`
    /// - `/foo`
    /// - `/users/123`
    ///
    /// If the incoming request matches the path exactly the corresponding service will
    /// be called.
    ///
    /// # Captures
    ///
    /// Paths can contain segments like `/{key}` which matches any single segment and
    /// will store the value captured at `key`. The value captured can be zero-length
    /// except for in the invalid path `//`.
    ///
    /// Examples:
    ///
    /// - `/{key}`
    /// - `/users/{id}`
    /// - `/users/{id}/tweets`
    ///
    /// Captures can be extracted using [`Path`](crate::extract::Path). See its
    /// documentation for more details.
    ///
    /// It is not possible to create segments that only match some types like numbers or
    /// regular expression. You must handle that manually in your handlers.
    ///
    /// [`MatchedPath`] can be used to extract the matched path rather than the actual path.
    ///
    /// # Wildcards
    ///
    /// Paths can end in `/{*key}` which matches all segments and will store the segments
    /// captured at `key`.
    ///
    /// Examples:
    ///
    /// - `/{*key}`
    /// - `/assets/{*path}`
    /// - `/{id}/{repo}/{*tree}`
    ///
    /// Note that `/{*key}` doesn't match empty segments. Thus:
    ///
    /// - `/{*key}` doesn't match `/` but does match `/a`, `/a/`, etc.
    /// - `/x/{*key}` doesn't match `/x` or `/x/` but does match `/x/a`, `/x/a/`, etc.
    ///
    /// Wildcard captures can also be extracted using [`Path`](crate::extract::Path):
    ///
    /// ```rust
    /// use axum::{
    ///     Router,
    ///     routing::get,
    ///     extract::Path,
    /// };
    ///
    /// let app: Router = Router::new().route("/{*key}", get(handler));
    ///
    /// async fn handler(Path(path): Path<String>) -> String {
    ///     path
    /// }
    /// ```
    ///
    /// Note that the leading slash is not included, i.e. for the route `/foo/{*rest}` and
    /// the path `/foo/bar/baz` the value of `rest` will be `bar/baz`.
    ///
    /// # Accepting multiple methods
    ///
    /// To accept multiple methods for the same route you can add all handlers at the
    /// same time:
    ///
    /// ```rust
    /// use axum::{Router, routing::{get, delete}, extract::Path};
    ///
    /// let app = Router::new().route(
    ///     "/",
    ///     get(get_root).post(post_root).delete(delete_root),
    /// );
    ///
    /// async fn get_root() {}
    ///
    /// async fn post_root() {}
    ///
    /// async fn delete_root() {}
    /// # let _: Router = app;
    /// ```
    ///
    /// Or you can add them one by one:
    ///
    /// ```rust
    /// # use axum::Router;
    /// # use axum::routing::{get, post, delete};
    /// #
    /// let app = Router::new()
    ///     .route("/", get(get_root))
    ///     .route("/", post(post_root))
    ///     .route("/", delete(delete_root));
    /// #
    /// # let _: Router = app;
    /// # async fn get_root() {}
    /// # async fn post_root() {}
    /// # async fn delete_root() {}
    /// ```
    ///
    /// # More examples
    ///
    /// ```rust
    /// use axum::{Router, routing::{get, delete}, extract::Path};
    ///
    /// let app = Router::new()
    ///     .route("/", get(root))
    ///     .route("/users", get(list_users).post(create_user))
    ///     .route("/users/{id}", get(show_user))
    ///     .route("/api/{version}/users/{id}/action", delete(do_users_action))
    ///     .route("/assets/{*path}", get(serve_asset));
    ///
    /// async fn root() {}
    ///
    /// async fn list_users() {}
    ///
    /// async fn create_user() {}
    ///
    /// async fn show_user(Path(id): Path<u64>) {}
    ///
    /// async fn do_users_action(Path((version, id)): Path<(String, u64)>) {}
    ///
    /// async fn serve_asset(Path(path): Path<String>) {}
    /// # let _: Router = app;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the route overlaps with another route:
    ///
    /// ```rust,should_panic
    /// use axum::{routing::get, Router};
    ///
    /// let app = Router::new()
    ///     .route("/", get(|| async {}))
    ///     .route("/", get(|| async {}));
    /// # let _: Router = app;
    /// ```
    ///
    /// The static route `/foo` and the dynamic route `/{key}` are not considered to
    /// overlap and `/foo` will take precedence.
    ///
    /// Also panics if `path` is empty.
    #[track_caller]
    pub fn route(self, path: &str, method_router: MethodRouter<S>) -> Self {
        tap_inner!(self, mut this => {
            panic_on_err!(this.path_router.route(path, method_router));
        })
    }

    /// Add another route to the router that calls a [`Service`].
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use axum::{
    ///     Router,
    ///     body::Body,
    ///     routing::{any_service, get_service},
    ///     extract::Request,
    ///     http::StatusCode,
    ///     error_handling::HandleErrorLayer,
    /// };
    /// use tower_http::services::ServeFile;
    /// use http::Response;
    /// use std::{convert::Infallible, io};
    /// use tower::service_fn;
    ///
    /// let app = Router::new()
    ///     .route(
    ///         // Any request to `/` goes to a service
    ///         "/",
    ///         // Services whose response body is not `axum::body::BoxBody`
    ///         // can be wrapped in `axum::routing::any_service` (or one of the other routing filters)
    ///         // to have the response body mapped
    ///         any_service(service_fn(|_: Request| async {
    ///             let res = Response::new(Body::from("Hi from `GET /`"));
    ///             Ok::<_, Infallible>(res)
    ///         }))
    ///     )
    ///     .route_service(
    ///         "/foo",
    ///         // This service's response body is `axum::body::BoxBody` so
    ///         // it can be routed to directly.
    ///         service_fn(|req: Request| async move {
    ///             let body = Body::from(format!("Hi from `{} /foo`", req.method()));
    ///             let res = Response::new(body);
    ///             Ok::<_, Infallible>(res)
    ///         })
    ///     )
    ///     .route_service(
    ///         // GET `/static/Cargo.toml` goes to a service from tower-http
    ///         "/static/Cargo.toml",
    ///         ServeFile::new("Cargo.toml"),
    ///     );
    /// # let _: Router = app;
    /// ```
    ///
    /// Routing to arbitrary services in this way has complications for backpressure
    /// ([`Service::poll_ready`]). See the [Routing to services and backpressure] module
    /// for more details.
    ///
    /// # Panics
    ///
    /// Panics for the same reasons as [`Router::route`] or if you attempt to route to a
    /// `Router`:
    ///
    /// ```rust,should_panic
    /// use axum::{routing::get, Router};
    ///
    /// let app = Router::new().route_service(
    ///     "/",
    ///     Router::new().route("/foo", get(|| async {})),
    /// );
    /// # let _: Router = app;
    /// ```
    ///
    /// Use [`Router::nest`] instead.
    ///
    /// [Routing to services and backpressure]: middleware/index.html#routing-to-servicesmiddleware-and-backpressure
    pub fn route_service<T>(self, path: &str, service: T) -> Self
    where
        T: Service<Request, Error = Infallible> + Clone + Send + Sync + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        let Err(service) = try_downcast::<Self, _>(service) else {
            panic!(
                "Invalid route: `Router::route_service` cannot be used with `Router`s. \
                Use `Router::nest` instead"
            );
        };

        tap_inner!(self, mut this => {
            panic_on_err!(this.path_router.route_service(path, service));
        })
    }

    /// Nest a [`Router`] at some path.
    ///
    /// This allows you to break your application into smaller pieces and compose
    /// them together.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::{
    ///     routing::{get, post},
    ///     Router,
    /// };
    ///
    /// let user_routes = Router::new().route("/{id}", get(|| async {}));
    ///
    /// let team_routes = Router::new().route("/", post(|| async {}));
    ///
    /// let api_routes = Router::new()
    ///     .nest("/users", user_routes)
    ///     .nest("/teams", team_routes);
    ///
    /// let app = Router::new().nest("/api", api_routes);
    ///
    /// // Our app now accepts
    /// // - GET /api/users/{id}
    /// // - POST /api/teams
    /// # let _: Router = app;
    /// ```
    ///
    /// # How the URI changes
    ///
    /// Note that nested routes will not see the original request URI but instead
    /// have the matched prefix stripped. This is necessary for services like static
    /// file serving to work. Use [`OriginalUri`] if you need the original request
    /// URI.
    ///
    /// # Captures from outer routes
    ///
    /// Take care when using `nest` together with dynamic routes as nesting also
    /// captures from the outer routes:
    ///
    /// ```rust
    /// use axum::{
    ///     extract::Path,
    ///     routing::get,
    ///     Router,
    /// };
    /// use std::collections::HashMap;
    ///
    /// async fn users_get(Path(params): Path<HashMap<String, String>>) {
    ///     // Both `version` and `id` were captured even though `users_api` only
    ///     // explicitly captures `id`.
    ///     let version = params.get("version");
    ///     let id = params.get("id");
    /// }
    ///
    /// let users_api = Router::new().route("/users/{id}", get(users_get));
    ///
    /// let app = Router::new().nest("/{version}/api", users_api);
    /// # let _: Router = app;
    /// ```
    ///
    /// # Differences from wildcard routes
    ///
    /// Nested routes are similar to wildcard routes. The difference is that
    /// wildcard routes still see the whole URI whereas nested routes will have
    /// the prefix stripped:
    ///
    /// ```rust
    /// use axum::{routing::get, http::Uri, Router};
    ///
    /// let nested_router = Router::new()
    ///     .route("/", get(|uri: Uri| async {
    ///         // `uri` will _not_ contain `/bar`
    ///     }));
    ///
    /// let app = Router::new()
    ///     .route("/foo/{*rest}", get(|uri: Uri| async {
    ///         // `uri` will contain `/foo`
    ///     }))
    ///     .nest("/bar", nested_router);
    /// # let _: Router = app;
    /// ```
    ///
    /// Additionally, while the wildcard route `/foo/*rest` will not match the
    /// paths `/foo` or `/foo/`, a nested router at `/foo` will match the path `/foo`
    /// (but not `/foo/`), and a nested router at `/foo/` will match the path `/foo/`
    /// (but not `/foo`).
    ///
    /// # Fallbacks
    ///
    /// If a nested router doesn't have its own fallback then it will inherit the
    /// fallback from the outer router:
    ///
    /// ```rust
    /// use axum::{routing::get, http::StatusCode, handler::Handler, Router};
    ///
    /// async fn fallback() -> (StatusCode, &'static str) {
    ///     (StatusCode::NOT_FOUND, "Not Found")
    /// }
    ///
    /// let api_routes = Router::new().route("/users", get(|| async {}));
    ///
    /// let app = Router::new()
    ///     .nest("/api", api_routes)
    ///     .fallback(fallback);
    /// # let _: Router = app;
    /// ```
    ///
    /// Here requests like `GET /api/not-found` will go into `api_routes` but because
    /// it doesn't have a matching route and doesn't have its own fallback it will call
    /// the fallback from the outer router, i.e. the `fallback` function.
    ///
    /// If the nested router has its own fallback then the outer fallback will not be
    /// inherited:
    ///
    /// ```rust
    /// use axum::{
    ///     routing::get,
    ///     http::StatusCode,
    ///     handler::Handler,
    ///     Json,
    ///     Router,
    /// };
    ///
    /// async fn fallback() -> (StatusCode, &'static str) {
    ///     (StatusCode::NOT_FOUND, "Not Found")
    /// }
    ///
    /// async fn api_fallback() -> (StatusCode, Json<serde_json::Value>) {
    ///     (
    ///         StatusCode::NOT_FOUND,
    ///         Json(serde_json::json!({ "status": "Not Found" })),
    ///     )
    /// }
    ///
    /// let api_routes = Router::new()
    ///     .route("/users", get(|| async {}))
    ///     .fallback(api_fallback);
    ///
    /// let app = Router::new()
    ///     .nest("/api", api_routes)
    ///     .fallback(fallback);
    /// # let _: Router = app;
    /// ```
    ///
    /// Here requests like `GET /api/not-found` will go to `api_fallback`.
    ///
    /// # Nesting routers with state
    ///
    /// When combining [`Router`]s with this method, each [`Router`] must have the
    /// same type of state. If your routers have different types you can use
    /// [`Router::with_state`] to provide the state and make the types match:
    ///
    /// ```rust
    /// use axum::{
    ///     Router,
    ///     routing::get,
    ///     extract::State,
    /// };
    ///
    /// #[derive(Clone)]
    /// struct InnerState {}
    ///
    /// #[derive(Clone)]
    /// struct OuterState {}
    ///
    /// async fn inner_handler(state: State<InnerState>) {}
    ///
    /// let inner_router = Router::new()
    ///     .route("/bar", get(inner_handler))
    ///     .with_state(InnerState {});
    ///
    /// async fn outer_handler(state: State<OuterState>) {}
    ///
    /// let app = Router::new()
    ///     .route("/", get(outer_handler))
    ///     .nest("/foo", inner_router)
    ///     .with_state(OuterState {});
    /// # let _: axum::Router = app;
    /// ```
    ///
    /// Note that the inner router will still inherit the fallback from the outer
    /// router.
    ///
    /// # Panics
    ///
    /// - If the route overlaps with another route. See [`Router::route`]
    ///   for more details.
    /// - If the route contains a wildcard (`*`).
    /// - If `path` is empty.
    ///
    /// [`OriginalUri`]: crate::extract::OriginalUri
    /// [fallbacks]: Router::fallback
    #[doc(alias = "scope")] // Some web frameworks like actix-web use this term
    #[track_caller]
    pub fn nest(self, path: &str, router: Self) -> Self {
        if path.is_empty() || path == "/" {
            panic!("Nesting at the root is no longer supported. Use merge instead.");
        }

        let RouterInner {
            path_router,
            default_fallback: _,
            // we don't need to inherit the catch-all fallback. It is only used for CONNECT
            // requests with an empty path. If we were to inherit the catch-all fallback
            // it would end up matching `/{path}/*` which doesn't match empty paths.
            catch_all_fallback: _,
        } = router.into_inner();

        tap_inner!(self, mut this => {
            panic_on_err!(this.path_router.nest(path, path_router));
        })
    }

    /// Like [`nest`](Self::nest), but accepts an arbitrary `Service`.
    #[track_caller]
    pub fn nest_service<T>(self, path: &str, service: T) -> Self
    where
        T: Service<Request, Error = Infallible> + Clone + Send + Sync + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        if path.is_empty() || path == "/" {
            panic!("Nesting at the root is no longer supported. Use fallback_service instead.");
        }

        tap_inner!(self, mut this => {
            panic_on_err!(this.path_router.nest_service(path, service));
        })
    }

    /// Merge the paths and fallbacks of two routers into a single [`Router`].
    ///
    /// This is useful for breaking apps into smaller pieces and combining them
    /// into one.
    ///
    /// ```rust
    /// use axum::{
    ///     routing::get,
    ///     Router,
    /// };
    /// #
    /// # async fn users_list() {}
    /// # async fn users_show() {}
    /// # async fn teams_list() {}
    ///
    /// // define some routes separately
    /// let user_routes = Router::new()
    ///     .route("/users", get(users_list))
    ///     .route("/users/{id}", get(users_show));
    ///
    /// let team_routes = Router::new()
    ///     .route("/teams", get(teams_list));
    ///
    /// // combine them into one
    /// let app = Router::new()
    ///     .merge(user_routes)
    ///     .merge(team_routes);
    ///
    /// // could also do `user_routes.merge(team_routes)`
    ///
    /// // Our app now accepts
    /// // - GET /users
    /// // - GET /users/{id}
    /// // - GET /teams
    /// # let _: Router = app;
    /// ```
    ///
    /// # Merging routers with state
    ///
    /// When combining [`Router`]s with this method, each [`Router`] must have the
    /// same type of state. If your routers have different types you can use
    /// [`Router::with_state`] to provide the state and make the types match:
    ///
    /// ```rust
    /// use axum::{
    ///     Router,
    ///     routing::get,
    ///     extract::State,
    /// };
    ///
    /// #[derive(Clone)]
    /// struct InnerState {}
    ///
    /// #[derive(Clone)]
    /// struct OuterState {}
    ///
    /// async fn inner_handler(state: State<InnerState>) {}
    ///
    /// let inner_router = Router::new()
    ///     .route("/bar", get(inner_handler))
    ///     .with_state(InnerState {});
    ///
    /// async fn outer_handler(state: State<OuterState>) {}
    ///
    /// let app = Router::new()
    ///     .route("/", get(outer_handler))
    ///     .merge(inner_router)
    ///     .with_state(OuterState {});
    /// # let _: axum::Router = app;
    /// ```
    ///
    /// # Merging routers with fallbacks
    ///
    /// When combining [`Router`]s with this method, the [fallback](Router::fallback) is also merged.
    /// However only one of the routers can have a fallback.
    ///
    /// # Panics
    ///
    /// - If two routers that each have a [fallback](Router::fallback) are merged. This
    ///   is because `Router` only allows a single fallback.
    #[track_caller]
    pub fn merge<R>(self, other: R) -> Self
    where
        R: Into<Self>,
    {
        let other: Self = other.into();
        let RouterInner {
            path_router,
            default_fallback,
            catch_all_fallback,
        } = other.into_inner();

        map_inner!(self, mut this => {
            match (this.default_fallback, default_fallback) {
                // other has a default fallback
                // use the one from other
                (_, true) => {}
                // this has default fallback, other has a custom fallback
                (true, false) => {
                    this.default_fallback = false;
                }
                // both have a custom fallback, not allowed
                (false, false) => {
                    panic!("Cannot merge two `Router`s that both have a fallback")
                }
            };

            panic_on_err!(this.path_router.merge(path_router));

            this.catch_all_fallback = this
                .catch_all_fallback
                .merge(catch_all_fallback)
                .unwrap_or_else(|| panic!("Cannot merge two `Router`s that both have a fallback"));

            this
        })
    }

    /// Apply a [`tower::Layer`] to all routes in the router.
    ///
    /// This can be used to add additional processing to a request for a group
    /// of routes.
    ///
    /// Note that the middleware is only applied to existing routes. So you have to
    /// first add your routes (and / or fallback) and then call `layer` afterwards. Additional
    /// routes added after `layer` is called will not have the middleware added.
    ///
    /// If you want to add middleware to a single handler you can either use
    /// [`MethodRouter::layer`] or [`Handler::layer`].
    ///
    /// # Example
    ///
    /// Adding the [`tower_http::trace::TraceLayer`]:
    ///
    /// ```rust
    /// use axum::{routing::get, Router};
    /// use tower_http::trace::TraceLayer;
    ///
    /// let app = Router::new()
    ///     .route("/foo", get(|| async {}))
    ///     .route("/bar", get(|| async {}))
    ///     .layer(TraceLayer::new_for_http());
    /// # let _: Router = app;
    /// ```
    ///
    /// If you need to write your own middleware see ["Writing
    /// middleware"](crate::middleware#writing-middleware) for the different options.
    ///
    /// If you only want middleware on some routes you can use [`Router::merge`]:
    ///
    /// ```rust
    /// use axum::{routing::get, Router};
    /// use tower_http::{trace::TraceLayer, compression::CompressionLayer};
    ///
    /// let with_tracing = Router::new()
    ///     .route("/foo", get(|| async {}))
    ///     .layer(TraceLayer::new_for_http());
    ///
    /// let with_compression = Router::new()
    ///     .route("/bar", get(|| async {}))
    ///     .layer(CompressionLayer::new());
    ///
    /// // Merge everything into one `Router`
    /// let app = Router::new()
    ///     .merge(with_tracing)
    ///     .merge(with_compression);
    /// # let _: Router = app;
    /// ```
    ///
    /// # Multiple middleware
    ///
    /// It's recommended to use [`tower::ServiceBuilder`] when applying multiple
    /// middleware. See [`middleware`](crate::middleware) for more details.
    ///
    /// # Runs after routing
    ///
    /// Middleware added with this method will run _after_ routing and thus cannot be
    /// used to rewrite the request URI. See ["Rewriting request URI in
    /// middleware"](crate::middleware#rewriting-request-uri-in-middleware) for more
    /// details and a workaround.
    ///
    /// # Error handling
    ///
    /// See [`middleware`](crate::middleware) for details on how error handling impacts
    /// middleware.
    pub fn layer<L>(self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + Send + Sync + 'static,
        L::Service: Service<Request> + Clone + Send + Sync + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
    {
        map_inner!(self, this => RouterInner {
            path_router: this.path_router.layer(layer.clone()),
            default_fallback: this.default_fallback,
            catch_all_fallback: this.catch_all_fallback.map(|route| route.layer(layer)),
        })
    }

    /// Apply a [`tower::Layer`] to the router that will only run if the request matches
    /// a route.
    ///
    /// Note that the middleware is only applied to existing routes. So you have to
    /// first add your routes (and / or fallback) and then call `route_layer`
    /// afterwards. Additional routes added after `route_layer` is called will not have
    /// the middleware added.
    ///
    /// This works similarly to [`Router::layer`] except the middleware will only run if
    /// the request matches a route. This is useful for middleware that return early
    /// (such as authorization) which might otherwise convert a `404 Not Found` into a
    /// `401 Unauthorized`.
    ///
    /// This function will panic if no routes have been declared yet on the router,
    /// since the new layer will have no effect, and this is typically a bug.
    /// In generic code, you can test if that is the case first, by calling [`Router::has_routes`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::{
    ///     routing::get,
    ///     Router,
    /// };
    /// use tower_http::validate_request::ValidateRequestHeaderLayer;
    ///
    /// let app = Router::new()
    ///     .route("/foo", get(|| async {}))
    ///     .route_layer(ValidateRequestHeaderLayer::bearer("password"));
    ///
    /// // `GET /foo` with a valid token will receive `200 OK`
    /// // `GET /foo` with a invalid token will receive `401 Unauthorized`
    /// // `GET /not-found` with a invalid token will receive `404 Not Found`
    /// # let _: Router = app;
    /// ```
    #[track_caller]
    pub fn route_layer<L>(self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + Send + Sync + 'static,
        L::Service: Service<Request> + Clone + Send + Sync + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
    {
        map_inner!(self, this => RouterInner {
            path_router: this.path_router.route_layer(layer),
            default_fallback: this.default_fallback,
            catch_all_fallback: this.catch_all_fallback,
        })
    }

    /// True if the router currently has at least one route added.
    #[must_use]
    pub fn has_routes(&self) -> bool {
        self.inner.path_router.has_routes()
    }

    /// Add a fallback [`Handler`] to the router.
    ///
    /// This service will be called if no routes matches the incoming request.
    ///
    /// ```rust
    /// use axum::{
    ///     Router,
    ///     routing::get,
    ///     handler::Handler,
    ///     response::IntoResponse,
    ///     http::{StatusCode, Uri},
    /// };
    ///
    /// let app = Router::new()
    ///     .route("/foo", get(|| async { /* ... */ }))
    ///     .fallback(fallback);
    ///
    /// async fn fallback(uri: Uri) -> (StatusCode, String) {
    ///     (StatusCode::NOT_FOUND, format!("No route for {uri}"))
    /// }
    /// # let _: Router = app;
    /// ```
    ///
    /// Fallbacks only apply to routes that aren't matched by anything in the
    /// router. If a handler is matched by a request but returns 404 the
    /// fallback is not called. Note that this applies to [`MethodRouter`]s too: if the
    /// request hits a valid path but the [`MethodRouter`] does not have an appropriate
    /// method handler installed, the fallback is not called (use
    /// [`MethodRouter::fallback`] for this purpose instead).
    ///
    ///
    /// # Handling all requests without other routes
    ///
    /// Using `Router::new().fallback(...)` to accept all request regardless of path or
    /// method, if you don't have other routes, isn't optimal:
    ///
    /// ```rust
    /// use axum::Router;
    ///
    /// async fn handler() {}
    ///
    /// let app = Router::new().fallback(handler);
    ///
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, app).await.unwrap();
    /// # };
    /// ```
    ///
    /// Running the handler directly is faster since it avoids the overhead of routing:
    ///
    /// ```rust
    /// use axum::handler::HandlerWithoutStateExt;
    ///
    /// async fn handler() {}
    ///
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, handler.into_make_service()).await.unwrap();
    /// # };
    /// ```
    #[track_caller]
    pub fn fallback<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        tap_inner!(self, mut this => {
            this.catch_all_fallback =
                Fallback::BoxedHandler(BoxedIntoRoute::from_handler(handler.clone()));
        })
        .fallback_endpoint(Endpoint::MethodRouter(any(handler)))
    }

    /// Add a fallback [`Service`] to the router.
    ///
    /// See [`Router::fallback`] for more details.
    pub fn fallback_service<T>(self, service: T) -> Self
    where
        T: Service<Request, Error = Infallible> + Clone + Send + Sync + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        let route = Route::new(service);
        tap_inner!(self, mut this => {
            this.catch_all_fallback = Fallback::Service(route.clone());
        })
        .fallback_endpoint(Endpoint::Route(route))
    }

    /// Add a fallback [`Handler`] for the case where a route exists, but the method of the request is not supported.
    ///
    /// Sets a fallback on all previously registered [`MethodRouter`]s,
    /// to be called when no matching method handler is set.
    ///
    /// ```rust,no_run
    /// use axum::{response::IntoResponse, routing::get, Router};
    ///
    /// async fn hello_world() -> impl IntoResponse {
    ///     "Hello, world!\n"
    /// }
    ///
    /// async fn default_fallback() -> impl IntoResponse {
    ///     "Default fallback\n"
    /// }
    ///
    /// async fn handle_405() -> impl IntoResponse {
    ///     "Method not allowed fallback"
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let router = Router::new()
    ///         .route("/", get(hello_world))
    ///         .fallback(default_fallback)
    ///         .method_not_allowed_fallback(handle_405);
    ///
    ///     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    ///
    ///     axum::serve(listener, router).await.unwrap();
    /// }
    /// ```
    ///
    /// The fallback only applies if there is a `MethodRouter` registered for a given path,
    /// but the method used in the request is not specified. In the example, a `GET` on
    /// `http://localhost:3000` causes the `hello_world` handler to react, while issuing a
    /// `POST` triggers `handle_405`. Calling an entirely different route, like `http://localhost:3000/hello`
    /// causes `default_fallback` to run.
    #[allow(clippy::needless_pass_by_value)]
    pub fn method_not_allowed_fallback<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        tap_inner!(self, mut this => {
            this.path_router
                .method_not_allowed_fallback(&handler);
        })
    }

    /// Reset the fallback to its default.
    ///
    /// Useful to merge two routers with fallbacks, as [`merge`] doesn't allow
    /// both routers to have an explicit fallback. Use this method to remove the
    /// one you want to discard before merging.
    ///
    /// [`merge`]: Self::merge
    pub fn reset_fallback(self) -> Self {
        tap_inner!(self, mut this => {
            this.default_fallback = true;
            this.catch_all_fallback = Fallback::Default(Route::new(NotFound));
        })
    }

    fn fallback_endpoint(self, endpoint: Endpoint<S>) -> Self {
        // TODO make this better, get rid of the `unwrap`s.
        // We need the returned `Service` to be `Clone` and the function inside `service_fn` to be
        // `FnMut` so instead of just using the owned service, we do this trick with `Option`. We
        // know this will be called just once so it's fine. We're doing that so that we avoid one
        // clone inside `oneshot_inner` so that the `Router` and subsequently the `State` is not
        // cloned too much.
        tap_inner!(self, mut this => {
            _ = this.path_router.route_endpoint(
                "/",
                endpoint.clone().layer(
                    layer_fn(
                        |service: Route| {
                            let mut service = Some(service);
                            service_fn(
                                #[cfg_attr(not(feature = "matched-path"), allow(unused_mut))]
                                move |mut request: Request| {
                                    #[cfg(feature = "matched-path")]
                                    request.extensions_mut().remove::<MatchedPath>();
                                    service.take().unwrap().oneshot_inner_owned(request)
                                }
                            )
                        }
                    )
                )
            );

            _ = this.path_router.route_endpoint(
                FALLBACK_PARAM_PATH,
                endpoint.layer(
                    layer_fn(
                        |service: Route| {
                            let mut service = Some(service);
                            service_fn(
                                #[cfg_attr(not(feature = "matched-path"), allow(unused_mut))]
                                move |mut request: Request| {
                                    #[cfg(feature = "matched-path")]
                                    request.extensions_mut().remove::<MatchedPath>();
                                    service.take().unwrap().oneshot_inner_owned(request)
                                }
                            )
                        }
                    )
                )
            );

            this.default_fallback = false;
        })
    }

    /// Provide the state for the router. State passed to this method is global and will be used
    /// for all requests this router receives. That means it is not suitable for holding state derived from a request, such as authorization data extracted in a middleware. Use [`Extension`] instead for such data.
    ///
    /// ```rust
    /// use axum::{Router, routing::get, extract::State};
    ///
    /// #[derive(Clone)]
    /// struct AppState {}
    ///
    /// let routes = Router::new()
    ///     .route("/", get(|State(state): State<AppState>| async {
    ///         // use state
    ///     }))
    ///     .with_state(AppState {});
    ///
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, routes).await.unwrap();
    /// # };
    /// ```
    ///
    /// # Returning routers with states from functions
    ///
    /// When returning `Router`s from functions, it is generally recommended not to set the
    /// state directly:
    ///
    /// ```rust
    /// use axum::{Router, routing::get, extract::State};
    ///
    /// #[derive(Clone)]
    /// struct AppState {}
    ///
    /// // Don't call `Router::with_state` here
    /// fn routes() -> Router<AppState> {
    ///     Router::new()
    ///         .route("/", get(|_: State<AppState>| async {}))
    /// }
    ///
    /// // Instead do it before you run the server
    /// let routes = routes().with_state(AppState {});
    ///
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, routes).await.unwrap();
    /// # };
    /// ```
    ///
    /// If you do need to provide the state, and you're _not_ nesting/merging the router
    /// into another router, then return `Router` without any type parameters:
    ///
    /// ```rust
    /// # use axum::{Router, routing::get, extract::State};
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// #
    /// // Don't return `Router<AppState>`
    /// fn routes(state: AppState) -> Router {
    ///     Router::new()
    ///         .route("/", get(|_: State<AppState>| async {}))
    ///         .with_state(state)
    /// }
    ///
    /// let routes = routes(AppState {});
    ///
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, routes).await.unwrap();
    /// # };
    /// ```
    ///
    /// This is because we can only call `Router::into_make_service` on `Router<()>`,
    /// not `Router<AppState>`. See below for more details about why that is.
    ///
    /// Note that the state defaults to `()` so `Router` and `Router<()>` is the same.
    ///
    /// If you are nesting/merging the router it is recommended to use a generic state
    /// type on the resulting router:
    ///
    /// ```rust
    /// # use axum::{Router, routing::get, extract::State};
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// #
    /// fn routes<S>(state: AppState) -> Router<S> {
    ///     Router::new()
    ///         .route("/", get(|_: State<AppState>| async {}))
    ///         .with_state(state)
    /// }
    ///
    /// let routes = Router::new().nest("/api", routes(AppState {}));
    ///
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, routes).await.unwrap();
    /// # };
    /// ```
    ///
    /// # What `S` in `Router<S>` means
    ///
    /// `Router<S>` means a router that is _missing_ a state of type `S` to be able to
    /// handle requests. It does _not_ mean a `Router` that _has_ a state of type `S`.
    ///
    /// For example:
    ///
    /// ```rust
    /// # use axum::{Router, routing::get, extract::State};
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// #
    /// // A router that _needs_ an `AppState` to handle requests
    /// let router: Router<AppState> = Router::new()
    ///     .route("/", get(|_: State<AppState>| async {}));
    ///
    /// // Once we call `Router::with_state` the router isn't missing
    /// // the state anymore, because we just provided it
    /// //
    /// // Therefore the router type becomes `Router<()>`, i.e a router
    /// // that is not missing any state
    /// let router: Router<()> = router.with_state(AppState {});
    ///
    /// // Only `Router<()>` has the `into_make_service` method.
    /// //
    /// // You cannot call `into_make_service` on a `Router<AppState>`
    /// // because it is still missing an `AppState`.
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, router).await.unwrap();
    /// # };
    /// ```
    ///
    /// Perhaps a little counter intuitively, `Router::with_state` doesn't always return a
    /// `Router<()>`. Instead you get to pick what the new missing state type is:
    ///
    /// ```rust
    /// # use axum::{Router, routing::get, extract::State};
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// #
    /// let router: Router<AppState> = Router::new()
    ///     .route("/", get(|_: State<AppState>| async {}));
    ///
    /// // When we call `with_state` we're able to pick what the next missing state type is.
    /// // Here we pick `String`.
    /// let string_router: Router<String> = router.with_state(AppState {});
    ///
    /// // That allows us to add new routes that uses `String` as the state type
    /// let string_router = string_router
    ///     .route("/needs-string", get(|_: State<String>| async {}));
    ///
    /// // Provide the `String` and choose `()` as the new missing state.
    /// let final_router: Router<()> = string_router.with_state("foo".to_owned());
    ///
    /// // Since we have a `Router<()>` we can run it.
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, final_router).await.unwrap();
    /// # };
    /// ```
    ///
    /// This why this returning `Router<AppState>` after calling `with_state` doesn't
    /// work:
    ///
    /// ```rust,compile_fail
    /// # use axum::{Router, routing::get, extract::State};
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// #
    /// // This won't work because we're returning a `Router<AppState>`
    /// // i.e. we're saying we're still missing an `AppState`
    /// fn routes(state: AppState) -> Router<AppState> {
    ///     Router::new()
    ///         .route("/", get(|_: State<AppState>| async {}))
    ///         .with_state(state)
    /// }
    ///
    /// let app = routes(AppState {});
    ///
    /// // We can only call `Router::into_make_service` on a `Router<()>`
    /// // but `app` is a `Router<AppState>`
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, app).await.unwrap();
    /// # };
    /// ```
    ///
    /// Instead return `Router<()>` since we have provided all the state needed:
    ///
    /// ```rust
    /// # use axum::{Router, routing::get, extract::State};
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// #
    /// // We've provided all the state necessary so return `Router<()>`
    /// fn routes(state: AppState) -> Router<()> {
    ///     Router::new()
    ///         .route("/", get(|_: State<AppState>| async {}))
    ///         .with_state(state)
    /// }
    ///
    /// let app = routes(AppState {});
    ///
    /// // We can now call `Router::into_make_service`
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, app).await.unwrap();
    /// # };
    /// ```
    ///
    /// # A note about performance
    ///
    /// If you need a `Router` that implements `Service` but you don't need any state (perhaps
    /// you're making a library that uses axum internally) then it is recommended to call this
    /// method before you start serving requests:
    ///
    /// ```rust
    /// use axum::{Router, routing::get};
    ///
    /// let app = Router::new()
    ///     .route("/", get(|| async { /* ... */ }))
    ///     // even though we don't need any state, call `with_state(())` anyway
    ///     .with_state(());
    /// # let _: Router = app;
    /// ```
    ///
    /// This is not required but it gives axum a chance to update some internals in the router
    /// which may impact performance and reduce allocations.
    ///
    /// Note that [`Router::into_make_service`] and [`Router::into_make_service_with_connect_info`]
    /// do this automatically.
    ///
    /// [`Extension`]: crate::Extension
    pub fn with_state<S2>(self, state: S) -> Router<S2> {
        map_inner!(self, this => RouterInner {
            path_router: this.path_router.with_state(state.clone()),
            default_fallback: this.default_fallback,
            catch_all_fallback: this.catch_all_fallback.with_state(state),
        })
    }

    pub(crate) fn call_with_state(&self, req: Request, state: S) -> RouteFuture<Infallible> {
        let (req, state) = match self.inner.path_router.call_with_state(req, state) {
            Ok(future) => return future,
            Err((req, state)) => (req, state),
        };

        self.inner
            .catch_all_fallback
            .clone()
            .call_with_state(req, state)
    }

    /// Convert the router into a borrowed [`Service`] with a fixed request body type, to aid type
    /// inference.
    ///
    /// In some cases when calling methods from [`tower::ServiceExt`] on a [`Router`] you might get
    /// type inference errors along the lines of
    ///
    /// ```not_rust
    /// let response = router.ready().await?.call(request).await?;
    ///                       ^^^^^ cannot infer type for type parameter `B`
    /// ```
    ///
    /// This happens because `Router` implements [`Service`] with `impl<B> Service<Request<B>> for Router<()>`.
    ///
    /// For example:
    ///
    /// ```compile_fail
    /// use axum::{
    ///     Router,
    ///     routing::get,
    ///     http::Request,
    ///     body::Body,
    /// };
    /// use tower::{Service, ServiceExt};
    ///
    /// # async fn async_main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut router = Router::new().route("/", get(|| async {}));
    /// let request = Request::new(Body::empty());
    /// let response = router.ready().await?.call(request).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Calling `Router::as_service` fixes that:
    ///
    /// ```
    /// use axum::{
    ///     Router,
    ///     routing::get,
    ///     http::Request,
    ///     body::Body,
    /// };
    /// use tower::{Service, ServiceExt};
    ///
    /// # async fn async_main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut router = Router::new().route("/", get(|| async {}));
    /// let request = Request::new(Body::empty());
    /// let response = router.as_service().ready().await?.call(request).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// This is mainly used when calling `Router` in tests. It shouldn't be necessary when running
    /// the `Router` normally via [`Router::into_make_service`].
    pub fn as_service<B>(&mut self) -> RouterAsService<'_, B, S> {
        RouterAsService {
            router: self,
            _marker: PhantomData,
        }
    }

    /// Convert the router into an owned [`Service`] with a fixed request body type, to aid type
    /// inference.
    ///
    /// This is the same as [`Router::as_service`] instead it returns an owned [`Service`]. See
    /// that method for more details.
    #[must_use]
    pub fn into_service<B>(self) -> RouterIntoService<B, S> {
        RouterIntoService {
            router: self,
            _marker: PhantomData,
        }
    }
}

impl Router {
    /// Convert this router into a [`MakeService`], that is a [`Service`] whose
    /// response is another service.
    ///
    /// ```
    /// use axum::{
    ///     routing::get,
    ///     Router,
    /// };
    ///
    /// let app = Router::new().route("/", get(|| async { "Hi!" }));
    ///
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, app).await.unwrap();
    /// # };
    /// ```
    ///
    /// [`MakeService`]: tower::make::MakeService
    #[must_use]
    pub fn into_make_service(self) -> IntoMakeService<Self> {
        // call `Router::with_state` such that everything is turned into `Route` eagerly
        // rather than doing that per request
        IntoMakeService::new(self.with_state(()))
    }

    /// Convert this router into a [`MakeService`], that will store `C`'s
    /// associated `ConnectInfo` in a request extension such that [`ConnectInfo`]
    /// can extract it.
    ///
    /// This enables extracting things like the client's remote address.
    ///
    /// Extracting [`std::net::SocketAddr`] is supported out of the box:
    ///
    /// ```rust
    /// use axum::{
    ///     extract::ConnectInfo,
    ///     routing::get,
    ///     Router,
    /// };
    /// use std::net::SocketAddr;
    ///
    /// let app = Router::new().route("/", get(handler));
    ///
    /// async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
    ///     format!("Hello {addr}")
    /// }
    ///
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
    /// # };
    /// ```
    ///
    /// You can implement custom a [`Connected`] like so:
    ///
    /// ```rust
    /// use axum::{
    ///     extract::connect_info::{ConnectInfo, Connected},
    ///     routing::get,
    ///     serve::IncomingStream,
    ///     Router,
    /// };
    /// use tokio::net::TcpListener;
    ///
    /// let app = Router::new().route("/", get(handler));
    ///
    /// async fn handler(
    ///     ConnectInfo(my_connect_info): ConnectInfo<MyConnectInfo>,
    /// ) -> String {
    ///     format!("Hello {my_connect_info:?}")
    /// }
    ///
    /// #[derive(Clone, Debug)]
    /// struct MyConnectInfo {
    ///     // ...
    /// }
    ///
    /// impl Connected<IncomingStream<'_, TcpListener>> for MyConnectInfo {
    ///     fn connect_info(target: IncomingStream<'_, TcpListener>) -> Self {
    ///         MyConnectInfo {
    ///             // ...
    ///         }
    ///     }
    /// }
    ///
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, app.into_make_service_with_connect_info::<MyConnectInfo>()).await.unwrap();
    /// # };
    /// ```
    ///
    /// See the [unix domain socket example][uds] for an example of how to use
    /// this to collect UDS connection info.
    ///
    /// [`MakeService`]: tower::make::MakeService
    /// [`Connected`]: crate::extract::connect_info::Connected
    /// [`ConnectInfo`]: crate::extract::connect_info::ConnectInfo
    /// [uds]: https://github.com/tokio-rs/axum/blob/main/examples/unix-domain-socket/src/main.rs
    #[cfg(feature = "tokio")]
    #[must_use]
    pub fn into_make_service_with_connect_info<C>(self) -> IntoMakeServiceWithConnectInfo<Self, C> {
        // call `Router::with_state` such that everything is turned into `Route` eagerly
        // rather than doing that per request
        IntoMakeServiceWithConnectInfo::new(self.with_state(()))
    }
}

// for `axum::serve(listener, router)`
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
const _: () = {
    use crate::serve;

    impl<L> Service<serve::IncomingStream<'_, L>> for Router<()>
    where
        L: serve::Listener,
    {
        type Response = Self;
        type Error = Infallible;
        type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: serve::IncomingStream<'_, L>) -> Self::Future {
            // call `Router::with_state` such that everything is turned into `Route` eagerly
            // rather than doing that per request
            std::future::ready(Ok(self.clone().with_state(())))
        }
    }
};

impl<B> Service<Request<B>> for Router<()>
where
    B: HttpBody<Data = bytes::Bytes> + Send + 'static,
    B::Error: Into<axum_core::BoxError>,
{
    type Response = Response;
    type Error = Infallible;
    type Future = RouteFuture<Infallible>;

    #[inline]
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let req = req.map(Body::new);
        self.call_with_state(req, ())
    }
}

/// A [`Router`] converted into a borrowed [`Service`] with a fixed body type.
///
/// See [`Router::as_service`] for more details.
pub struct RouterAsService<'a, B, S = ()> {
    router: &'a mut Router<S>,
    _marker: PhantomData<B>,
}

impl<B> Service<Request<B>> for RouterAsService<'_, B, ()>
where
    B: HttpBody<Data = bytes::Bytes> + Send + 'static,
    B::Error: Into<axum_core::BoxError>,
{
    type Response = Response;
    type Error = Infallible;
    type Future = RouteFuture<Infallible>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <Router as Service<Request<B>>>::poll_ready(self.router, cx)
    }

    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        self.router.call(req)
    }
}

impl<B, S> fmt::Debug for RouterAsService<'_, B, S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouterAsService")
            .field("router", &self.router)
            .finish()
    }
}

/// A [`Router`] converted into an owned [`Service`] with a fixed body type.
///
/// See [`Router::into_service`] for more details.
pub struct RouterIntoService<B, S = ()> {
    router: Router<S>,
    _marker: PhantomData<B>,
}

impl<B, S> Clone for RouterIntoService<B, S>
where
    Router<S>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            router: self.router.clone(),
            _marker: PhantomData,
        }
    }
}

impl<B> Service<Request<B>> for RouterIntoService<B, ()>
where
    B: HttpBody<Data = bytes::Bytes> + Send + 'static,
    B::Error: Into<axum_core::BoxError>,
{
    type Response = Response;
    type Error = Infallible;
    type Future = RouteFuture<Infallible>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <Router as Service<Request<B>>>::poll_ready(&mut self.router, cx)
    }

    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        self.router.call(req)
    }
}

impl<B, S> fmt::Debug for RouterIntoService<B, S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouterIntoService")
            .field("router", &self.router)
            .finish()
    }
}

enum Fallback<S, E = Infallible> {
    Default(Route<E>),
    Service(Route<E>),
    BoxedHandler(BoxedIntoRoute<S, E>),
}

impl<S, E> Fallback<S, E>
where
    S: Clone,
{
    fn merge(self, other: Self) -> Option<Self> {
        match (self, other) {
            // If either are `Default`, return the opposite one.
            (Self::Default(_), pick) | (pick, Self::Default(_)) => Some(pick),
            // Otherwise, return None
            _ => None,
        }
    }

    fn map<F, E2>(self, f: F) -> Fallback<S, E2>
    where
        S: 'static,
        E: 'static,
        F: FnOnce(Route<E>) -> Route<E2> + Clone + Send + Sync + 'static,
        E2: 'static,
    {
        match self {
            Self::Default(route) => Fallback::Default(f(route)),
            Self::Service(route) => Fallback::Service(f(route)),
            Self::BoxedHandler(handler) => Fallback::BoxedHandler(handler.map(f)),
        }
    }

    fn with_state<S2>(self, state: S) -> Fallback<S2, E> {
        match self {
            Self::Default(route) => Fallback::Default(route),
            Self::Service(route) => Fallback::Service(route),
            Self::BoxedHandler(handler) => Fallback::Service(handler.into_route(state)),
        }
    }

    fn call_with_state(self, req: Request, state: S) -> RouteFuture<E> {
        match self {
            Self::Default(route) | Self::Service(route) => route.oneshot_inner_owned(req),
            Self::BoxedHandler(handler) => {
                let route = handler.into_route(state);
                route.oneshot_inner_owned(req)
            }
        }
    }
}

impl<S, E> Clone for Fallback<S, E> {
    fn clone(&self) -> Self {
        match self {
            Self::Default(inner) => Self::Default(inner.clone()),
            Self::Service(inner) => Self::Service(inner.clone()),
            Self::BoxedHandler(inner) => Self::BoxedHandler(inner.clone()),
        }
    }
}

impl<S, E> fmt::Debug for Fallback<S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default(inner) => f.debug_tuple("Default").field(inner).finish(),
            Self::Service(inner) => f.debug_tuple("Service").field(inner).finish(),
            Self::BoxedHandler(_) => f.debug_tuple("BoxedHandler").finish(),
        }
    }
}

#[allow(clippy::large_enum_variant)]
enum Endpoint<S> {
    MethodRouter(MethodRouter<S>),
    Route(Route),
}

impl<S> Endpoint<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn layer<L>(self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + Send + Sync + 'static,
        L::Service: Service<Request> + Clone + Send + Sync + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
    {
        match self {
            Self::MethodRouter(method_router) => Self::MethodRouter(method_router.layer(layer)),
            Self::Route(route) => Self::Route(route.layer(layer)),
        }
    }
}

impl<S> Clone for Endpoint<S> {
    fn clone(&self) -> Self {
        match self {
            Self::MethodRouter(inner) => Self::MethodRouter(inner.clone()),
            Self::Route(inner) => Self::Route(inner.clone()),
        }
    }
}

impl<S> fmt::Debug for Endpoint<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MethodRouter(method_router) => {
                f.debug_tuple("MethodRouter").field(method_router).finish()
            }
            Self::Route(route) => f.debug_tuple("Route").field(route).finish(),
        }
    }
}

#[test]
fn traits() {
    use crate::test_helpers::*;
    assert_send::<Router<()>>();
    assert_sync::<Router<()>>();
}
