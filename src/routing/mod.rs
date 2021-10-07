//! Routing between [`Service`]s.

use self::future::{BoxRouteFuture, EmptyRouterFuture, NestedFuture, RouteFuture};
use crate::{
    body::{box_body, BoxBody},
    clone_box_service::CloneBoxService,
    extract::{
        connect_info::{Connected, IntoMakeServiceWithConnectInfo},
        OriginalUri,
    },
    service::HandleError,
    util::{ByteStr, PercentDecodedByteStr},
    BoxError,
};
use bytes::Bytes;
use http::{Request, Response, StatusCode, Uri};
use matchit::Node;
use std::{
    borrow::Cow,
    convert::Infallible,
    fmt,
    future::ready,
    marker::PhantomData,
    task::{Context, Poll},
};
use tower::{util::ServiceExt, ServiceBuilder};
use tower_http::map_response_body::MapResponseBodyLayer;
use tower_layer::Layer;
use tower_service::Service;

pub mod future;

mod method_filter;
mod or;

pub use self::{method_filter::MethodFilter, or::Or};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RouteId(u64);

impl RouteId {
    fn next() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static ID: AtomicU64 = AtomicU64::new(0);
        Self(ID.fetch_add(1, Ordering::SeqCst))
    }
}

/// The router type for composing handlers and services.
#[derive(Clone)]
pub struct Router<S> {
    routes: S,
    node: Node<RouteId>,
}

impl<E> Router<EmptyRouter<E>> {
    /// Create a new `Router`.
    ///
    /// Unless you add additional routes this will respond to `404 Not Found` to
    /// all requests.
    pub fn new() -> Self {
        Self {
            routes: EmptyRouter::not_found(),
            node: Node::new(),
        }
    }
}

impl<E> Default for Router<EmptyRouter<E>> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> fmt::Debug for Router<S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Router")
            .field("routes", &self.routes)
            .finish()
    }
}

const NEST_TAIL_PARAM: &str = "__axum_nest";

impl<S> Router<S> {
    /// Add another route to the router.
    ///
    /// `path` is a string of path segments separated by `/`. Each segment
    /// can be either concrete or a capture:
    ///
    /// - `/foo/bar/baz` will only match requests where the path is `/foo/bar/bar`.
    /// - `/:foo` will match any route with exactly one segment _and_ it will
    /// capture the first segment and store it at the key `foo`.
    ///
    /// `service` is the [`Service`] that should receive the request if the path
    /// matches `path`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::{handler::{get, delete}, Router};
    ///
    /// let app = Router::new()
    ///     .route("/", get(root))
    ///     .route("/users", get(list_users).post(create_user))
    ///     .route("/users/:id", get(show_user))
    ///     .route("/api/:version/users/:id/action", delete(do_thing));
    ///
    /// async fn root() { /* ... */ }
    ///
    /// async fn list_users() { /* ... */ }
    ///
    /// async fn create_user() { /* ... */ }
    ///
    /// async fn show_user() { /* ... */ }
    ///
    /// async fn do_thing() { /* ... */ }
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the route overlaps with another route:
    ///
    /// ```should_panic
    /// use axum::{handler::get, Router};
    ///
    /// let app = Router::new()
    ///     .route("/", get(|| async {}))
    ///     .route("/", get(|| async {}));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// This also applies to `nest` which is similar to a wildcard route:
    ///
    /// ```should_panic
    /// use axum::{handler::get, Router};
    ///
    /// let app = Router::new()
    ///     // this is similar to `/api/*`
    ///     .nest("/api", get(|| async {}))
    ///     // which overlaps with this route
    ///     .route("/api/users", get(|| async {}));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// Note that routes like `/:key` and `/foo` are considered overlapping:
    ///
    /// ```should_panic
    /// use axum::{handler::get, Router};
    ///
    /// let app = Router::new()
    ///     .route("/foo", get(|| async {}))
    ///     .route("/:key", get(|| async {}));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn route<T>(mut self, path: &str, svc: T) -> Router<Route<T, S>> {
        let id = RouteId::next();

        if let Err(err) = self.node.insert(path, id) {
            panic!("Invalid route: {}", err);
        }

        Router {
            routes: Route {
                id,
                svc,
                fallback: self.routes,
            },
            node: self.node,
        }
    }

    /// Nest a group of routes (or a [`Service`]) at some path.
    ///
    /// This allows you to break your application into smaller pieces and compose
    /// them together.
    ///
    /// ```
    /// use axum::{
    ///     handler::get,
    ///     Router,
    /// };
    /// use http::Uri;
    ///
    /// async fn users_get(uri: Uri) {
    ///     // `uri` will be `/users` since `nest` strips the matching prefix.
    ///     // use `OriginalUri` to always get the full URI.
    /// }
    ///
    /// async fn users_post() {}
    ///
    /// async fn careers() {}
    ///
    /// let users_api = Router::new().route("/users", get(users_get).post(users_post));
    ///
    /// let app = Router::new()
    ///     .nest("/api", users_api)
    ///     .route("/careers", get(careers));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// Note that nested routes will not see the orignal request URI but instead
    /// have the matched prefix stripped. This is necessary for services like static
    /// file serving to work. Use [`OriginalUri`] if you need the original request
    /// URI.
    ///
    /// Take care when using `nest` together with dynamic routes as nesting also
    /// captures from the outer routes:
    ///
    /// ```
    /// use axum::{
    ///     extract::Path,
    ///     handler::get,
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
    /// let users_api = Router::new().route("/users/:id", get(users_get));
    ///
    /// let app = Router::new().nest("/:version/api", users_api);
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// `nest` also accepts any [`Service`]. This can for example be used with
    /// [`tower_http::services::ServeDir`] to serve static files from a directory:
    ///
    /// ```
    /// use axum::{
    ///     Router,
    ///     service::get,
    /// };
    /// use tower_http::services::ServeDir;
    ///
    /// // Serves files inside the `public` directory at `GET /public/*`
    /// let serve_dir_service = ServeDir::new("public");
    ///
    /// let app = Router::new().nest("/public", get(serve_dir_service));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// If necessary you can use [`Router::boxed`] to box a group of routes
    /// making the type easier to name. This is sometimes useful when working with
    /// `nest`.
    ///
    /// # Wildcard routes
    ///
    /// Nested routes are similar to wildcard routes. The difference is that
    /// wildcard routes still see the whole URI whereas nested routes will have
    /// the prefix stripped.
    ///
    /// ```rust
    /// use axum::{handler::get, http::Uri, Router};
    ///
    /// let app = Router::new()
    ///     .route("/foo/*rest", get(|uri: Uri| async {
    ///         // `uri` will contain `/foo`
    ///     }))
    ///     .nest("/bar", get(|uri: Uri| async {
    ///         // `uri` will _not_ contain `/bar`
    ///     }));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the route overlaps with another route. See [`Router::route`]
    /// for more details.
    ///
    /// [`OriginalUri`]: crate::extract::OriginalUri
    pub fn nest<T>(mut self, path: &str, svc: T) -> Router<Nested<T, S>> {
        let id = RouteId::next();

        let path = if path == "/" {
            format!("/*{}", NEST_TAIL_PARAM)
        } else {
            format!("{}/*{}", path, NEST_TAIL_PARAM)
        };

        if let Err(err) = self.node.insert(path, id) {
            panic!("Invalid route: {}", err);
        }

        Router {
            routes: Nested {
                id,
                svc,
                fallback: self.routes,
            },
            node: self.node,
        }
    }

    /// Create a boxed route trait object.
    ///
    /// This makes it easier to name the types of routers to, for example,
    /// return them from functions:
    ///
    /// ```rust
    /// use axum::{
    ///     body::Body,
    ///     handler::get,
    ///     Router,
    ///     routing::BoxRoute
    /// };
    ///
    /// async fn first_handler() { /* ... */ }
    ///
    /// async fn second_handler() { /* ... */ }
    ///
    /// async fn third_handler() { /* ... */ }
    ///
    /// fn app() -> Router<BoxRoute> {
    ///     Router::new()
    ///         .route("/", get(first_handler).post(second_handler))
    ///         .route("/foo", get(third_handler))
    ///         .boxed()
    /// }
    /// ```
    ///
    /// It also helps with compile times when you have a very large number of
    /// routes.
    pub fn boxed<ReqBody, ResBody>(self) -> Router<BoxRoute<ReqBody, S::Error>>
    where
        S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + Sync + 'static,
        S::Error: Into<BoxError> + Send,
        S::Future: Send,
        ReqBody: Send + 'static,
        ResBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        ResBody::Error: Into<BoxError>,
    {
        self.map(|svc| {
            ServiceBuilder::new()
                .layer_fn(BoxRoute)
                .layer_fn(CloneBoxService::new)
                .layer(MapResponseBodyLayer::new(box_body))
                .service(svc)
        })
    }

    /// Apply a [`tower::Layer`] to the router.
    ///
    /// All requests to the router will be processed by the layer's
    /// corresponding middleware.
    ///
    /// This can be used to add additional processing to a request for a group
    /// of routes.
    ///
    /// Note this differs from [`handler::Layered`](crate::handler::Layered)
    /// which adds a middleware to a single handler.
    ///
    /// # Example
    ///
    /// Adding the [`tower::limit::ConcurrencyLimit`] middleware to a group of
    /// routes can be done like so:
    ///
    /// ```rust
    /// use axum::{
    ///     handler::get,
    ///     Router,
    /// };
    /// use tower::limit::{ConcurrencyLimitLayer, ConcurrencyLimit};
    ///
    /// async fn first_handler() { /* ... */ }
    ///
    /// async fn second_handler() { /* ... */ }
    ///
    /// async fn third_handler() { /* ... */ }
    ///
    /// // All requests to `handler` and `other_handler` will be sent through
    /// // `ConcurrencyLimit`
    /// let app = Router::new().route("/", get(first_handler))
    ///     .route("/foo", get(second_handler))
    ///     .layer(ConcurrencyLimitLayer::new(64))
    ///     // Request to `GET /bar` will go directly to `third_handler` and
    ///     // wont be sent through `ConcurrencyLimit`
    ///     .route("/bar", get(third_handler));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// This is commonly used to add middleware such as tracing/logging to your
    /// entire app:
    ///
    /// ```rust
    /// use axum::{
    ///     handler::get,
    ///     Router,
    /// };
    /// use tower_http::trace::TraceLayer;
    ///
    /// async fn first_handler() { /* ... */ }
    ///
    /// async fn second_handler() { /* ... */ }
    ///
    /// async fn third_handler() { /* ... */ }
    ///
    /// let app = Router::new()
    ///     .route("/", get(first_handler))
    ///     .route("/foo", get(second_handler))
    ///     .route("/bar", get(third_handler))
    ///     .layer(TraceLayer::new_for_http());
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn layer<L>(self, layer: L) -> Router<Layered<L::Service>>
    where
        L: Layer<S>,
    {
        self.map(|svc| Layered::new(layer.layer(svc)))
    }

    /// Convert this router into a [`MakeService`], that is a [`Service`] who's
    /// response is another service.
    ///
    /// This is useful when running your application with hyper's
    /// [`Server`](hyper::server::Server):
    ///
    /// ```
    /// use axum::{
    ///     handler::get,
    ///     Router,
    /// };
    ///
    /// let app = Router::new().route("/", get(|| async { "Hi!" }));
    ///
    /// # async {
    /// axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
    ///     .serve(app.into_make_service())
    ///     .await
    ///     .expect("server failed");
    /// # };
    /// ```
    ///
    /// [`MakeService`]: tower::make::MakeService
    pub fn into_make_service(self) -> IntoMakeService<Self>
    where
        S: Clone,
    {
        IntoMakeService::new(self)
    }

    /// Convert this router into a [`MakeService`], that will store `C`'s
    /// associated `ConnectInfo` in a request extension such that [`ConnectInfo`]
    /// can extract it.
    ///
    /// This enables extracting things like the client's remote address.
    ///
    /// Extracting [`std::net::SocketAddr`] is supported out of the box:
    ///
    /// ```
    /// use axum::{
    ///     extract::ConnectInfo,
    ///     handler::get,
    ///     Router,
    /// };
    /// use std::net::SocketAddr;
    ///
    /// let app = Router::new().route("/", get(handler));
    ///
    /// async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
    ///     format!("Hello {}", addr)
    /// }
    ///
    /// # async {
    /// axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
    ///     .serve(
    ///         app.into_make_service_with_connect_info::<SocketAddr, _>()
    ///     )
    ///     .await
    ///     .expect("server failed");
    /// # };
    /// ```
    ///
    /// You can implement custom a [`Connected`] like so:
    ///
    /// ```
    /// use axum::{
    ///     extract::connect_info::{ConnectInfo, Connected},
    ///     handler::get,
    ///     Router,
    /// };
    /// use hyper::server::conn::AddrStream;
    ///
    /// let app = Router::new().route("/", get(handler));
    ///
    /// async fn handler(
    ///     ConnectInfo(my_connect_info): ConnectInfo<MyConnectInfo>,
    /// ) -> String {
    ///     format!("Hello {:?}", my_connect_info)
    /// }
    ///
    /// #[derive(Clone, Debug)]
    /// struct MyConnectInfo {
    ///     // ...
    /// }
    ///
    /// impl Connected<&AddrStream> for MyConnectInfo {
    ///     type ConnectInfo = MyConnectInfo;
    ///
    ///     fn connect_info(target: &AddrStream) -> Self::ConnectInfo {
    ///         MyConnectInfo {
    ///             // ...
    ///         }
    ///     }
    /// }
    ///
    /// # async {
    /// axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
    ///     .serve(
    ///         app.into_make_service_with_connect_info::<MyConnectInfo, _>()
    ///     )
    ///     .await
    ///     .expect("server failed");
    /// # };
    /// ```
    ///
    /// See the [unix domain socket example][uds] for an example of how to use
    /// this to collect UDS connection info.
    ///
    /// [`MakeService`]: tower::make::MakeService
    /// [`Connected`]: crate::extract::connect_info::Connected
    /// [`ConnectInfo`]: crate::extract::connect_info::ConnectInfo
    /// [uds]: https://github.com/tokio-rs/axum/blob/main/examples/unix_domain_socket.rs
    pub fn into_make_service_with_connect_info<C, Target>(
        self,
    ) -> IntoMakeServiceWithConnectInfo<Self, C>
    where
        S: Clone,
        C: Connected<Target>,
    {
        IntoMakeServiceWithConnectInfo::new(self)
    }

    /// Merge two routers into one.
    ///
    /// This is useful for breaking apps into smaller pieces and combining them
    /// into one.
    ///
    /// ```
    /// use axum::{
    ///     handler::get,
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
    ///     .route("/users/:id", get(users_show));
    ///
    /// let team_routes = Router::new().route("/teams", get(teams_list));
    ///
    /// // combine them into one
    /// let app = user_routes.or(team_routes);
    /// # async {
    /// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn or<S2>(self, other: S2) -> Router<Or<S, S2>> {
        self.map(|first| Or {
            first,
            second: other,
        })
    }

    /// Handle errors services in this router might produce, by mapping them to
    /// responses.
    ///
    /// Unhandled errors will close the connection without sending a response.
    ///
    /// # Example
    ///
    /// ```
    /// use axum::{
    ///     handler::get,
    ///     http::StatusCode,
    ///     Router,
    /// };
    /// use tower::{BoxError, timeout::TimeoutLayer};
    /// use std::{time::Duration, convert::Infallible};
    ///
    /// // This router can never fail, since handlers can never fail.
    /// let app = Router::new().route("/", get(|| async {}));
    ///
    /// // Now the router can fail since the `tower::timeout::Timeout`
    /// // middleware will return an error if the timeout elapses.
    /// let app = app.layer(TimeoutLayer::new(Duration::from_secs(10)));
    ///
    /// // With `handle_error` we can handle errors `Timeout` might produce.
    /// // Our router now cannot fail, that is its error type is `Infallible`.
    /// let app = app.handle_error(|error: BoxError| {
    ///     if error.is::<tower::timeout::error::Elapsed>() {
    ///         Ok::<_, Infallible>((
    ///             StatusCode::REQUEST_TIMEOUT,
    ///             "request took too long to handle".to_string(),
    ///         ))
    ///     } else {
    ///         Ok::<_, Infallible>((
    ///             StatusCode::INTERNAL_SERVER_ERROR,
    ///             format!("Unhandled error: {}", error),
    ///         ))
    ///     }
    /// });
    /// # async {
    /// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// You can return `Err(_)` from the closure if you don't wish to handle
    /// some errors:
    ///
    /// ```
    /// use axum::{
    ///     handler::get,
    ///     http::StatusCode,
    ///     Router,
    /// };
    /// use tower::{BoxError, timeout::TimeoutLayer};
    /// use std::time::Duration;
    ///
    /// let app = Router::new()
    ///     .route("/", get(|| async {}))
    ///     .layer(TimeoutLayer::new(Duration::from_secs(10)))
    ///     .handle_error(|error: BoxError| {
    ///         if error.is::<tower::timeout::error::Elapsed>() {
    ///             Ok((
    ///                 StatusCode::REQUEST_TIMEOUT,
    ///                 "request took too long to handle".to_string(),
    ///             ))
    ///         } else {
    ///             // return the error as is
    ///             Err(error)
    ///         }
    ///     });
    /// # async {
    /// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn handle_error<ReqBody, F>(self, f: F) -> Router<HandleError<S, F, ReqBody>> {
        self.map(|svc| HandleError::new(svc, f))
    }

    /// Check that your service cannot fail.
    ///
    /// That is, its error type is [`Infallible`].
    pub fn check_infallible(self) -> Router<CheckInfallible<S>> {
        self.map(CheckInfallible)
    }

    fn map<F, S2>(self, f: F) -> Router<S2>
    where
        F: FnOnce(S) -> S2,
    {
        Router {
            routes: f(self.routes),
            node: self.node,
        }
    }
}

impl<ReqBody, ResBody, S> Service<Request<ReqBody>> for Router<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone,
    ReqBody: Send + Sync + 'static,
{
    type Response = Response<ResBody>;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.routes.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        if req.extensions().get::<OriginalUri>().is_none() {
            let original_uri = OriginalUri(req.uri().clone());
            req.extensions_mut().insert(original_uri);
        }

        let path = req.uri().path().to_string();
        if let Ok(match_) = self.node.at(&path) {
            let id = *match_.value;
            req.extensions_mut().insert(id);

            let params = match_
                .params
                .iter()
                .filter(|(key, _)| !key.starts_with(NEST_TAIL_PARAM))
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect::<Vec<_>>();

            if let Some(tail) = match_.params.get(NEST_TAIL_PARAM) {
                UriStack::push(&mut req);
                let new_uri = with_path(req.uri(), tail);
                *req.uri_mut() = new_uri;
            }

            insert_url_params(&mut req, params);
        }

        self.routes.call(req)
    }
}

pub(crate) struct UriStack(Vec<Uri>);

impl UriStack {
    fn push<B>(req: &mut Request<B>) {
        let uri = req.uri().clone();

        if let Some(stack) = req.extensions_mut().get_mut::<Self>() {
            stack.0.push(uri);
        } else {
            req.extensions_mut().insert(Self(vec![uri]));
        }
    }

    pub(crate) fn pop<B>(req: &mut Request<B>) -> Option<Uri> {
        req.extensions_mut()
            .get_mut::<Self>()
            .and_then(|stack| stack.0.pop())
    }
}

// we store the potential error here such that users can handle invalid path
// params using `Result<Path<T>, _>`. That wouldn't be possible if we
// returned an error immediately when decoding the param
pub(crate) struct UrlParams(
    pub(crate) Result<Vec<(ByteStr, PercentDecodedByteStr)>, InvalidUtf8InPathParam>,
);

fn insert_url_params<B>(req: &mut Request<B>, params: Vec<(String, String)>) {
    let params = params
        .into_iter()
        .map(|(k, v)| {
            if let Some(decoded) = PercentDecodedByteStr::new(v) {
                Ok((ByteStr::new(k), decoded))
            } else {
                Err(InvalidUtf8InPathParam {
                    key: ByteStr::new(k),
                })
            }
        })
        .collect::<Result<Vec<_>, _>>();

    if let Some(current) = req.extensions_mut().get_mut::<Option<UrlParams>>() {
        match params {
            Ok(params) => {
                let mut current = current.take().unwrap();
                if let Ok(current) = &mut current.0 {
                    current.extend(params);
                }
                req.extensions_mut().insert(Some(current));
            }
            Err(err) => {
                req.extensions_mut().insert(Some(UrlParams(Err(err))));
            }
        }
    } else {
        req.extensions_mut().insert(Some(UrlParams(params)));
    }
}

pub(crate) struct InvalidUtf8InPathParam {
    pub(crate) key: ByteStr,
}

/// A [`Service`] that responds with `404 Not Found` or `405 Method not allowed`
/// to all requests.
///
/// This is used as the bottom service in a router stack. You shouldn't have to
/// use it manually.
pub struct EmptyRouter<E = Infallible> {
    status: StatusCode,
    _marker: PhantomData<fn() -> E>,
}

impl<E> EmptyRouter<E> {
    pub(crate) fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            _marker: PhantomData,
        }
    }

    pub(crate) fn method_not_allowed() -> Self {
        Self {
            status: StatusCode::METHOD_NOT_ALLOWED,
            _marker: PhantomData,
        }
    }
}

impl<E> Clone for EmptyRouter<E> {
    fn clone(&self) -> Self {
        Self {
            status: self.status,
            _marker: PhantomData,
        }
    }
}

impl<E> fmt::Debug for EmptyRouter<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EmptyRouter").finish()
    }
}

impl<B, E> Service<Request<B>> for EmptyRouter<E>
where
    B: Send + Sync + 'static,
{
    type Response = Response<BoxBody>;
    type Error = E;
    type Future = EmptyRouterFuture<E>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut request: Request<B>) -> Self::Future {
        if self.status == StatusCode::METHOD_NOT_ALLOWED {
            // we're inside a route but there was no method that matched
            // so record that so we can override the status if no other
            // routes match
            request.extensions_mut().insert(NoMethodMatch);
        }

        if self.status == StatusCode::NOT_FOUND
            && request.extensions().get::<NoMethodMatch>().is_some()
        {
            self.status = StatusCode::METHOD_NOT_ALLOWED;
        }

        let mut res = Response::new(crate::body::empty());

        res.extensions_mut().insert(FromEmptyRouter { request });

        *res.status_mut() = self.status;
        EmptyRouterFuture {
            future: ready(Ok(res)),
        }
    }
}

#[derive(Clone, Copy)]
struct NoMethodMatch;

/// Response extension used by [`EmptyRouter`] to send the request back to [`Or`] so
/// the other service can be called.
///
/// Without this we would loose ownership of the request when calling the first
/// service in [`Or`]. We also wouldn't be able to identify if the response came
/// from [`EmptyRouter`] and therefore can be discarded in [`Or`].
struct FromEmptyRouter<B> {
    request: Request<B>,
}

/// A route that sends requests to one of two [`Service`]s depending on the
/// path.
#[derive(Debug, Clone)]
pub struct Route<S, T> {
    id: RouteId,
    svc: S,
    fallback: T,
}

impl<B, S, T> Service<Request<B>> for Route<S, T>
where
    S: Service<Request<B>, Response = Response<BoxBody>> + Clone,
    T: Service<Request<B>, Response = Response<BoxBody>, Error = S::Error> + Clone,
    B: Send + Sync + 'static,
{
    type Response = Response<BoxBody>;
    type Error = S::Error;
    type Future = RouteFuture<S, T, B>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        match req.extensions().get::<RouteId>() {
            Some(id) => {
                if self.id == *id {
                    RouteFuture::a(self.svc.clone().oneshot(req))
                } else {
                    RouteFuture::b(self.fallback.clone().oneshot(req))
                }
            }
            None => RouteFuture::b(self.fallback.clone().oneshot(req)),
        }
    }
}

/// A [`Service`] that has been nested inside a router at some path.
///
/// Created with [`Router::nest`].
#[derive(Debug, Clone)]
pub struct Nested<S, T> {
    id: RouteId,
    svc: S,
    fallback: T,
}

impl<B, S, T> Service<Request<B>> for Nested<S, T>
where
    S: Service<Request<B>, Response = Response<BoxBody>> + Clone,
    T: Service<Request<B>, Response = Response<BoxBody>, Error = S::Error> + Clone,
    B: Send + Sync + 'static,
{
    type Response = Response<BoxBody>;
    type Error = S::Error;
    type Future = NestedFuture<S, T, B>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let future = match req.extensions().get::<RouteId>() {
            Some(id) => {
                if self.id == *id {
                    RouteFuture::a(self.svc.clone().oneshot(req))
                } else {
                    RouteFuture::b(self.fallback.clone().oneshot(req))
                }
            }
            None => RouteFuture::b(self.fallback.clone().oneshot(req)),
        };

        NestedFuture { inner: future }
    }
}

/// A boxed route trait object.
///
/// See [`Router::boxed`] for more details.
pub struct BoxRoute<B = crate::body::Body, E = Infallible>(
    CloneBoxService<Request<B>, Response<BoxBody>, E>,
);

impl<B, E> Clone for BoxRoute<B, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<B, E> fmt::Debug for BoxRoute<B, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxRoute").finish()
    }
}

impl<B, E> Service<Request<B>> for BoxRoute<B, E>
where
    E: Into<BoxError>,
{
    type Response = Response<BoxBody>;
    type Error = E;
    type Future = BoxRouteFuture<B, E>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        BoxRouteFuture {
            inner: self.0.clone().oneshot(req),
        }
    }
}

/// A [`Service`] created from a router by applying a Tower middleware.
///
/// Created with [`Router::layer`]. See that method for more details.
pub struct Layered<S> {
    inner: S,
}

impl<S> Layered<S> {
    fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S> Clone for Layered<S>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self::new(self.inner.clone())
    }
}

impl<S> fmt::Debug for Layered<S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Layered")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<S, R> Service<R> for Layered<S>
where
    S: Service<R>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: R) -> Self::Future {
        self.inner.call(req)
    }
}

fn with_path(uri: &Uri, new_path: &str) -> Uri {
    let path_and_query = if let Some(path_and_query) = uri.path_and_query() {
        let new_path = if new_path.starts_with('/') {
            Cow::Borrowed(new_path)
        } else {
            Cow::Owned(format!("/{}", new_path))
        };

        if let Some(query) = path_and_query.query() {
            Some(
                format!("{}?{}", new_path, query)
                    .parse::<http::uri::PathAndQuery>()
                    .unwrap(),
            )
        } else {
            Some(new_path.parse().unwrap())
        }
    } else {
        None
    };

    let mut parts = http::uri::Parts::default();
    parts.scheme = uri.scheme().cloned();
    parts.authority = uri.authority().cloned();
    parts.path_and_query = path_and_query;

    Uri::from_parts(parts).unwrap()
}

/// Middleware that statically verifies that a service cannot fail.
///
/// Created with [`check_infallible`](Router::check_infallible).
#[derive(Debug, Clone, Copy)]
pub struct CheckInfallible<S>(S);

impl<R, S> Service<R> for CheckInfallible<S>
where
    S: Service<R, Error = Infallible>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: R) -> Self::Future {
        self.0.call(req)
    }
}

/// A [`MakeService`] that produces axum router services.
///
/// [`MakeService`]: tower::make::MakeService
#[derive(Debug, Clone)]
pub struct IntoMakeService<S> {
    service: S,
}

impl<S> IntoMakeService<S> {
    fn new(service: S) -> Self {
        Self { service }
    }
}

impl<S, T> Service<T> for IntoMakeService<S>
where
    S: Clone,
{
    type Response = S;
    type Error = Infallible;
    type Future = future::MakeRouteServiceFuture<S>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _target: T) -> Self::Future {
        future::MakeRouteServiceFuture {
            future: ready(Ok(self.service.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traits() {
        use crate::tests::*;

        assert_send::<Router<()>>();
        assert_sync::<Router<()>>();

        assert_send::<Route<(), ()>>();
        assert_sync::<Route<(), ()>>();

        assert_send::<EmptyRouter<NotSendSync>>();
        assert_sync::<EmptyRouter<NotSendSync>>();

        assert_send::<BoxRoute<(), ()>>();
        assert_sync::<BoxRoute<(), ()>>();

        assert_send::<Layered<()>>();
        assert_sync::<Layered<()>>();

        assert_send::<Nested<(), ()>>();
        assert_sync::<Nested<(), ()>>();

        assert_send::<CheckInfallible<()>>();
        assert_sync::<CheckInfallible<()>>();

        assert_send::<IntoMakeService<()>>();
        assert_sync::<IntoMakeService<()>>();
    }
}
