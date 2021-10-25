//! Routing between [`Service`]s and handlers.

use self::future::{EmptyRouterFuture, NestedFuture, RouteFuture, RouterFuture};
use crate::{
    body::{box_body, Body, BoxBody},
    clone_box_service::CloneBoxService,
    extract::{
        connect_info::{Connected, IntoMakeServiceWithConnectInfo},
        OriginalUri,
    },
    util::{ByteStr, PercentDecodedByteStr},
    BoxError,
};
use bytes::Bytes;
use http::{Request, Response, StatusCode, Uri};
use std::{
    borrow::Cow,
    collections::HashMap,
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
pub mod handler_method_router;
pub mod service_method_router;

mod method_filter;

pub use self::method_filter::MethodFilter;

#[doc(no_inline)]
pub use self::handler_method_router::{
    any, connect, delete, get, head, on, options, patch, post, put, trace, MethodRouter,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct RouteId(u64);

impl RouteId {
    fn next() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static ID: AtomicU64 = AtomicU64::new(0);
        Self(ID.fetch_add(1, Ordering::SeqCst))
    }
}

/// The router type for composing handlers and services.
pub struct Router<B = Body> {
    routes: HashMap<RouteId, Route<B>>,
    node: Node,
    fallback: Option<Route<B>>,
}

impl<B> Clone for Router<B> {
    fn clone(&self) -> Self {
        Self {
            routes: self.routes.clone(),
            node: self.node.clone(),
            fallback: self.fallback.clone(),
        }
    }
}

impl<B> Default for Router<B>
where
    B: Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<B> fmt::Debug for Router<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Router")
            .field("routes", &self.routes)
            .field("node", &self.node)
            .field("fallback", &self.fallback)
            .finish()
    }
}

const NEST_TAIL_PARAM: &str = "__axum_internal_nest_capture";

impl<B> Router<B>
where
    B: Send + Sync + 'static,
{
    /// Create a new `Router`.
    ///
    /// Unless you add additional routes this will respond to `404 Not Found` to
    /// all requests.
    pub fn new() -> Self {
        Self {
            routes: Default::default(),
            node: Default::default(),
            fallback: None,
        }
    }

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
    /// use axum::{routing::{get, delete}, Router};
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
    /// use axum::{routing::get, Router};
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
    /// use axum::{routing::get, Router};
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
    /// use axum::{routing::get, Router};
    ///
    /// let app = Router::new()
    ///     .route("/foo", get(|| async {}))
    ///     .route("/:key", get(|| async {}));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn route<T>(mut self, path: &str, svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible>
            + Clone
            + Send
            + 'static,
        T::Future: Send + 'static,
    {
        let id = RouteId::next();

        if let Err(err) = self.node.insert(path, id) {
            panic!("Invalid route: {}", err);
        }

        self.routes.insert(id, Route(CloneBoxService::new(svc)));

        self
    }

    /// Nest a group of routes (or a [`Service`]) at some path.
    ///
    /// This allows you to break your application into smaller pieces and compose
    /// them together.
    ///
    /// ```
    /// use axum::{
    ///     routing::get,
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
    ///     routing::service_method_router::get,
    ///     error_handling::HandleErrorExt,
    ///     http::StatusCode,
    /// };
    /// use std::{io, convert::Infallible};
    /// use tower_http::services::ServeDir;
    ///
    /// // Serves files inside the `public` directory at `GET /public/*`
    /// let serve_dir_service = ServeDir::new("public")
    ///     .handle_error(|error: io::Error| {
    ///         (
    ///             StatusCode::INTERNAL_SERVER_ERROR,
    ///             format!("Unhandled internal error: {}", error),
    ///         )
    ///     });
    ///
    /// let app = Router::new().nest("/public", get(serve_dir_service));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// # Wildcard routes
    ///
    /// Nested routes are similar to wildcard routes. The difference is that
    /// wildcard routes still see the whole URI whereas nested routes will have
    /// the prefix stripped.
    ///
    /// ```rust
    /// use axum::{routing::get, http::Uri, Router};
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
    pub fn nest<T>(mut self, path: &str, svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible>
            + Clone
            + Send
            + 'static,
        T::Future: Send + 'static,
    {
        let id = RouteId::next();

        if path.contains('*') {
            panic!("Invalid route: nested routes cannot contain wildcards (*)");
        }

        let path = if path == "/" {
            format!("/*{}", NEST_TAIL_PARAM)
        } else {
            format!("{}/*{}", path, NEST_TAIL_PARAM)
        };

        if let Err(err) = self.node.insert(path, id) {
            panic!("Invalid route: {}", err);
        }

        self.routes
            .insert(id, Route(CloneBoxService::new(Nested { svc })));

        self
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
    ///     routing::get,
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
    ///     routing::get,
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
    pub fn layer<L, LayeredReqBody, LayeredResBody>(self, layer: L) -> Router<LayeredReqBody>
    where
        L: Layer<Route<B>>,
        L::Service: Service<
                Request<LayeredReqBody>,
                Response = Response<LayeredResBody>,
                Error = Infallible,
            > + Clone
            + Send
            + 'static,
        <L::Service as Service<Request<LayeredReqBody>>>::Future: Send + 'static,
        LayeredResBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        LayeredResBody::Error: Into<BoxError>,
    {
        let layer = ServiceBuilder::new()
            .layer_fn(Route)
            .layer_fn(CloneBoxService::new)
            .layer(MapResponseBodyLayer::new(box_body))
            .layer(layer);

        let routes = self
            .routes
            .into_iter()
            .map(|(id, route)| {
                let route = Layer::layer(&layer, route);
                (id, route)
            })
            .collect::<HashMap<RouteId, Route<LayeredReqBody>>>();

        let fallback = self.fallback.map(|fallback| Layer::layer(&layer, fallback));

        Router {
            routes,
            node: self.node,
            fallback,
        }
    }

    /// Convert this router into a [`MakeService`], that is a [`Service`] who's
    /// response is another service.
    ///
    /// This is useful when running your application with hyper's
    /// [`Server`](hyper::server::Server):
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
    /// axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
    ///     .serve(app.into_make_service())
    ///     .await
    ///     .expect("server failed");
    /// # };
    /// ```
    ///
    /// [`MakeService`]: tower::make::MakeService
    pub fn into_make_service(self) -> IntoMakeService<Self> {
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
    ///     routing::get,
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
    ///     routing::get,
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
    ///     fn connect_info(target: &AddrStream) -> Self {
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
    ///     .route("/users/:id", get(users_show));
    ///
    /// let team_routes = Router::new().route("/teams", get(teams_list));
    ///
    /// // combine them into one
    /// let app = user_routes.merge(team_routes);
    /// # async {
    /// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn merge(mut self, other: Router<B>) -> Self {
        let Router {
            routes,
            node,
            fallback,
        } = other;

        if let Err(err) = self.node.merge(node) {
            panic!("Invalid route: {}", err);
        }

        for (id, route) in routes {
            assert!(self.routes.insert(id, route).is_none());
        }

        if let Some(new_fallback) = fallback {
            self.fallback = Some(new_fallback);
        }

        self
    }

    /// ```
    /// todo!();
    /// ```
    pub fn fallback<T>(mut self, svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible>
            + Clone
            + Send
            + 'static,
        T::Future: Send + 'static,
    {
        self.fallback = Some(Route(CloneBoxService::new(svc)));
        self
    }

    #[inline]
    fn call_route(&self, match_: matchit::Match<&RouteId>, mut req: Request<B>) -> RouterFuture<B> {
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

        let route = self
            .routes
            .get(&id)
            .expect("no route for id. This is a bug in axum. Please file an issue")
            .clone();

        RouterFuture {
            future: futures_util::future::Either::Left(route.oneshot(req)),
        }
    }
}

impl<B> Service<Request<B>> for Router<B>
where
    B: Send + Sync + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = RouterFuture<B>;

    #[inline]
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        if req.extensions().get::<OriginalUri>().is_none() {
            let original_uri = OriginalUri(req.uri().clone());
            req.extensions_mut().insert(original_uri);
        }

        let path = req.uri().path().to_string();

        if let Ok(match_) = self.node.at(&path) {
            self.call_route(match_, req)
        } else if let Some(fallback) = &self.fallback {
            RouterFuture {
                future: futures_util::future::Either::Left(fallback.clone().oneshot(req)),
            }
        } else {
            let res = EmptyRouter::<Infallible>::not_found().call_sync(req);
            RouterFuture {
                future: futures_util::future::Either::Right(std::future::ready(Ok(res))),
            }
        }
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

    fn call_sync<B>(&mut self, _req: Request<B>) -> Response<BoxBody>
    where
        B: Send + Sync + 'static,
    {
        let mut res = Response::new(crate::body::empty());
        *res.status_mut() = self.status;
        res
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

    fn call(&mut self, request: Request<B>) -> Self::Future {
        let res = self.call_sync(request);

        EmptyRouterFuture {
            future: ready(Ok(res)),
        }
    }
}

/// A [`Service`] that has been nested inside a router at some path.
///
/// Created with [`Router::nest`].
#[derive(Debug, Clone)]
struct Nested<S> {
    svc: S,
}

impl<B, S> Service<Request<B>> for Nested<S>
where
    S: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible> + Clone,
    B: Send + Sync + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = NestedFuture<S, B>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        NestedFuture {
            inner: self.svc.clone().oneshot(req),
        }
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

/// How routes are stored inside a [`Router`].
///
/// You normally shouldn't need to care about this type.
pub struct Route<B = Body>(CloneBoxService<Request<B>, Response<BoxBody>, Infallible>);

impl<ReqBody> Clone for Route<ReqBody> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<ReqBody> fmt::Debug for Route<ReqBody> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Route").finish()
    }
}

impl<B> Service<Request<B>> for Route<B> {
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = RouteFuture;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        RouteFuture {
            future: self.0.call(req),
        }
    }
}

#[derive(Clone, Default)]
struct Node {
    inner: matchit::Node<RouteId>,
    paths: Vec<(String, RouteId)>,
}

impl Node {
    fn insert(
        &mut self,
        path: impl Into<String>,
        val: RouteId,
    ) -> Result<(), matchit::InsertError> {
        let path = path.into();
        self.inner.insert(&path, val)?;
        self.paths.push((path, val));
        Ok(())
    }

    fn merge(&mut self, other: Node) -> Result<(), matchit::InsertError> {
        for (path, id) in other.paths {
            self.insert(path, id)?;
        }
        Ok(())
    }

    fn at<'n, 'p>(
        &'n self,
        path: &'p str,
    ) -> Result<matchit::Match<'n, 'p, &'n RouteId>, matchit::MatchError> {
        self.inner.at(path)
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node").field("paths", &self.paths).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traits() {
        use crate::tests::*;

        assert_send::<Router<()>>();

        assert_send::<Route<()>>();

        assert_send::<EmptyRouter<NotSendSync>>();
        assert_sync::<EmptyRouter<NotSendSync>>();

        assert_send::<Nested<()>>();
        assert_sync::<Nested<()>>();

        assert_send::<IntoMakeService<()>>();
        assert_sync::<IntoMakeService<()>>();
    }
}
