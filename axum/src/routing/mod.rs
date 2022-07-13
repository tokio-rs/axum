//! Routing between [`Service`]s and handlers.

use self::{future::RouteFuture, not_found::NotFound};
use crate::{
    body::{boxed, Body, Bytes, HttpBody},
    extract::connect_info::IntoMakeServiceWithConnectInfo,
    response::Response,
    routing::strip_prefix::StripPrefix,
    util::try_downcast,
    BoxError,
};
use http::Request;
use matchit::MatchError;
use std::{
    borrow::Cow,
    collections::HashMap,
    convert::Infallible,
    fmt,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{layer::layer_fn, ServiceBuilder};
use tower_http::map_response_body::MapResponseBodyLayer;
use tower_layer::Layer;
use tower_service::Service;

pub mod future;

mod into_make_service;
mod method_filter;
mod method_routing;
mod not_found;
mod route;
mod strip_prefix;
pub(crate) mod url_params;

#[cfg(test)]
mod tests;

pub use self::{into_make_service::IntoMakeService, method_filter::MethodFilter, route::Route};

pub use self::method_routing::{
    any, any_service, delete, delete_service, get, get_service, head, head_service, on, on_service,
    options, options_service, patch, patch_service, post, post_service, put, put_service, trace,
    trace_service, MethodRouter,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct RouteId(u32);

impl RouteId {
    fn next() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        // `AtomicU64` isn't supported on all platforms
        static ID: AtomicU32 = AtomicU32::new(0);
        let id = ID.fetch_add(1, Ordering::Relaxed);
        if id == u32::MAX {
            panic!("Over `u32::MAX` routes created. If you need this, please file an issue.");
        }
        Self(id)
    }
}

pub(crate) const PRIVATE_PARAM_PREFIX: &str = "__private__axum";
pub(crate) const NEST_TAIL_PARAM: &str = "__private__axum_nest_tail_param";
pub(crate) const NEST_TAIL_PARAM_CAPTURE: &str = "/*__private__axum_nest_tail_param";
pub(crate) const NEST_FALLBACK_PARAM_CAPTURE: &str = "/*__private__axum_fallback";

/// The router type for composing handlers and services.
pub struct Router<B = Body> {
    routes: InnerRoutes<Endpoint<B>>,
    custom_fallbacks: InnerRoutes<Route<B>>,
    default_fallback: DefaultFallback<B>,
}

impl<B> Router<B>
where
    B: HttpBody + Send + 'static,
{
    /// Create a new `Router`.
    ///
    /// Unless you add additional routes this will respond with `404 Not Found` to
    /// all requests.
    pub fn new() -> Self {
        Self {
            routes: Default::default(),
            custom_fallbacks: Default::default(),
            default_fallback: DefaultFallback::Unaltered,
        }
    }

    #[doc = include_str!("../docs/routing/route.md")]
    pub fn route<T>(mut self, path: &str, service: T) -> Self
    where
        T: Service<Request<B>, Response = Response, Error = Infallible> + Clone + Send + 'static,
        T::Future: Send + 'static,
    {
        if path.is_empty() {
            panic!("Paths must start with a `/`. Use \"/\" for root routes");
        } else if !path.starts_with('/') {
            panic!("Paths must start with a `/`");
        }

        let service = match try_downcast::<Router<B>, _>(service) {
            Ok(_) => {
                panic!("Invalid route: `Router::route` cannot be used with `Router`s. Use `Router::nest` instead")
            }
            Err(svc) => svc,
        };

        let endpoint = match try_downcast::<MethodRouter<B, Infallible>, _>(service) {
            Ok(method_router) => {
                let prev_id_and_endpoint =
                    self.routes
                        .node
                        .path_to_route_id
                        .get(path)
                        .and_then(|route_id| {
                            self.routes
                                .route_id_to_endpoint
                                .get(route_id)
                                .map(|svc| (*route_id, svc))
                        });

                if let Some((route_id, Endpoint::MethodRouter(prev_method_router))) =
                    prev_id_and_endpoint
                {
                    // if we're adding a new `MethodRouter` to a route that already has one just
                    // merge them. This makes `.route("/", get(_)).route("/", post(_))` work
                    let method_router = prev_method_router.clone().merge(method_router);
                    self.routes
                        .route_id_to_endpoint
                        .insert(route_id, Endpoint::MethodRouter(method_router));
                    return self;
                } else {
                    Endpoint::MethodRouter(method_router)
                }
            }
            Err(service) => Endpoint::Route(Route::new(service)),
        };

        self.routes
            .try_insert(path, endpoint)
            .unwrap_or_else(|err| panic!("{}", err));

        self
    }

    #[doc = include_str!("../docs/routing/nest.md")]
    pub fn nest(mut self, mut path: &str, router: Router<B>) -> Self {
        fn make_full_path<'a>(nested_path: &'a str, path: &'a str) -> Cow<'a, str> {
            if nested_path == "/" {
                path.into()
            } else if path == "/" {
                nested_path.into()
            } else if let Some(path) = path.strip_suffix('/') {
                format!("{}{}", path, nested_path).into()
            } else {
                format!("{}{}", path, nested_path).into()
            }
        }

        if path.is_empty() {
            // nesting at `""` and `"/"` should mean the same thing
            path = "/";
        }

        if path.contains('*') {
            panic!("Invalid route: nested routes cannot contain wildcards (*)");
        }

        let prefix = path;

        let Router {
            routes,
            custom_fallbacks,
            // We consider the router on the right a "sub router" and therefore we always discards
            // its default fallback. Custom fallbacks will be nested.
            default_fallback: _,
        } = router;

        for (nested_path, route) in routes.into_iter() {
            let full_path = make_full_path(&nested_path, path);
            self = match route {
                Endpoint::MethodRouter(method_router) => self.route(
                    &full_path,
                    method_router.layer(layer_fn(|s| StripPrefix::new(s, prefix))),
                ),
                Endpoint::Route(route) => self.route(&full_path, StripPrefix::new(route, prefix)),
            };
        }

        let mut first = None;
        for (nested_path, route) in custom_fallbacks.into_iter() {
            first = Some(route.clone());
            let full_path = make_full_path(&nested_path, path);

            self.custom_fallbacks
                .try_insert(&full_path, route.clone())
                .unwrap();
        }

        if let Some(route) = first {
            if !prefix.ends_with('/') {
                self.custom_fallbacks.overwrite(prefix, route.clone());

                self.custom_fallbacks
                    .try_insert(&format!("{}/", prefix), route)
                    .unwrap();
            }
        }

        self
    }

    #[doc = include_str!("../docs/routing/nest_service.md")]
    pub fn nest_service<T>(mut self, mut path: &str, svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response, Error = Infallible> + Clone + Send + 'static,
        T::Future: Send + 'static,
    {
        if path.is_empty() {
            // nesting at `""` and `"/"` should mean the same thing
            path = "/";
        }

        if path.contains('*') {
            panic!("Invalid route: nested routes cannot contain wildcards (*)");
        }

        let prefix = path;

        let path = if path.ends_with('/') {
            format!("{}*{}", path, NEST_TAIL_PARAM)
        } else {
            format!("{}/*{}", path, NEST_TAIL_PARAM)
        };

        let svc = StripPrefix::new(svc, prefix);
        self = self.route(&path, svc.clone());

        // `/*rest` is not matched by `/` so we need to also register a router at the
        // prefix itself. Otherwise if you were to nest at `/foo` then `/foo` itself
        // wouldn't match, which it should
        self = self.route(prefix, svc.clone());
        if !prefix.ends_with('/') {
            // same goes for `/foo/`, that should also match
            self = self.route(&format!("{}/", prefix), svc);
        }

        self
    }

    #[doc = include_str!("../docs/routing/merge.md")]
    pub fn merge<R>(mut self, other: R) -> Self
    where
        R: Into<Router<B>>,
    {
        let Router {
            routes,
            custom_fallbacks,
            // We consider the router on the right a "sub router" and therefore we always discards
            // its default fallback. Custom fallbacks will be merged.
            default_fallback: _,
        } = other.into();

        for (path, route) in routes.into_iter() {
            self = match route {
                Endpoint::MethodRouter(route) => self.route(&path, route),
                Endpoint::Route(route) => self.route(&path, route),
            };
        }

        for (path, route) in custom_fallbacks.into_iter() {
            self.custom_fallbacks
                .try_insert(&path, route)
                .unwrap_or_else(|_| panic!("Cannot merge routers that both have fallbacks"));
        }

        self
    }

    #[doc = include_str!("../docs/routing/layer.md")]
    pub fn layer<L, NewReqBody, NewResBody>(self, layer: L) -> Router<NewReqBody>
    where
        L: Layer<Route<B>>,
        L::Service:
            Service<Request<NewReqBody>, Response = Response<NewResBody>> + Clone + Send + 'static,
        <L::Service as Service<Request<NewReqBody>>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request<NewReqBody>>>::Future: Send + 'static,
        NewResBody: HttpBody<Data = Bytes> + Send + 'static,
        NewResBody::Error: Into<BoxError>,
    {
        let layer = ServiceBuilder::new()
            .map_err(Into::into)
            .layer(MapResponseBodyLayer::new(boxed))
            .layer(layer)
            .into_inner();

        let routes = self.routes.map_routes(|route| match route {
            Endpoint::MethodRouter(method_router) => {
                Endpoint::MethodRouter(method_router.layer(&layer))
            }
            Endpoint::Route(route) => Endpoint::Route(Route::new(layer.layer(route))),
        });

        let custom_fallbacks = self
            .custom_fallbacks
            .map_routes(|route| Route::new(layer.layer(route)));

        let default_fallback = match self.default_fallback {
            DefaultFallback::Unaltered => {
                let not_found = Route::new(NotFound);
                DefaultFallback::Layered(Route::new(layer.layer(not_found)))
            }
            DefaultFallback::Layered(inner) => {
                DefaultFallback::Layered(Route::new(layer.layer(inner)))
            }
        };

        Router {
            routes,
            custom_fallbacks,
            default_fallback,
        }
    }

    #[doc = include_str!("../docs/routing/route_layer.md")]
    pub fn route_layer<L, NewResBody>(self, layer: L) -> Self
    where
        L: Layer<Route<B>>,
        L::Service: Service<Request<B>, Response = Response<NewResBody>> + Clone + Send + 'static,
        <L::Service as Service<Request<B>>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request<B>>>::Future: Send + 'static,
        NewResBody: HttpBody<Data = Bytes> + Send + 'static,
        NewResBody::Error: Into<BoxError>,
    {
        let layer = ServiceBuilder::new()
            .map_err(Into::into)
            .layer(MapResponseBodyLayer::new(boxed))
            .layer(layer)
            .into_inner();

        let routes = self.routes.map_routes(|route| match route {
            Endpoint::MethodRouter(method_router) => {
                Endpoint::MethodRouter(method_router.layer(&layer))
            }
            Endpoint::Route(route) => Endpoint::Route(Route::new(layer.layer(route))),
        });

        Router {
            routes,
            custom_fallbacks: self.custom_fallbacks,
            default_fallback: self.default_fallback,
        }
    }

    #[doc = include_str!("../docs/routing/fallback.md")]
    pub fn fallback<T>(mut self, service: T) -> Self
    where
        T: Service<Request<B>, Response = Response, Error = Infallible> + Clone + Send + 'static,
        T::Future: Send + 'static,
    {
        let route = Route::new(service);

        let result = (|| {
            self.custom_fallbacks.try_insert("/", route.clone())?;
            self.custom_fallbacks
                .try_insert(NEST_FALLBACK_PARAM_CAPTURE, route)?;
            Ok::<_, InsertError<'_, _>>(())
        })();

        if result.is_err() {
            panic!("Cannot set fallback twice");
        }

        self
    }

    /// Convert this router into a [`MakeService`], that is a [`Service`] whose
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

    #[doc = include_str!("../docs/routing/into_make_service_with_connect_info.md")]
    pub fn into_make_service_with_connect_info<C>(self) -> IntoMakeServiceWithConnectInfo<Self, C> {
        IntoMakeServiceWithConnectInfo::new(self)
    }
}

impl<B> Clone for Router<B> {
    fn clone(&self) -> Self {
        Self {
            routes: self.routes.clone(),
            custom_fallbacks: self.custom_fallbacks.clone(),
            default_fallback: self.default_fallback.clone(),
        }
    }
}

impl<B> Default for Router<B>
where
    B: HttpBody + Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<B> fmt::Debug for Router<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            routes,
            custom_fallbacks,
            default_fallback,
        } = self;

        f.debug_struct("Router")
            .field("routes", routes)
            .field("custom_fallbacks", custom_fallbacks)
            .field("default_fallback", default_fallback)
            .finish()
    }
}

impl<B> Service<Request<B>> for Router<B>
where
    B: HttpBody + Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = RouteFuture<B, Infallible>;

    #[inline]
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        #[cfg(feature = "original-uri")]
        crate::extract::request_parts::insert_original_uri(&mut req);

        // clone the uri rather than the path because the uri is ref counted so
        // probably cheaper to clone that than allocate a new string
        let uri = req.uri().clone();

        // check if a route matches
        if let Some(match_) = self.routes.at(uri.path()) {
            let Match {
                params,
                #[cfg(feature = "matched-path")]
                matched_path,
                route,
            } = match_;

            #[cfg(feature = "matched-path")]
            crate::extract::matched_path::insert_matched_path(matched_path, &mut req);

            url_params::insert_url_params(req.extensions_mut(), params);

            match &mut route.clone() {
                Endpoint::MethodRouter(inner) => return inner.call(req),
                Endpoint::Route(inner) => return inner.call(req),
            }
        }

        // check if a custom fallback matches
        if let Some(match_) = self.custom_fallbacks.at(uri.path()) {
            let Match {
                params,
                // don't set matched path because no path matched (:
                #[cfg(feature = "matched-path")]
                    matched_path: _,
                route,
            } = match_;

            url_params::insert_url_params(req.extensions_mut(), params);

            return route.clone().call(req);
        }

        // finally call the default fallback if nothing else matches
        match &self.default_fallback {
            DefaultFallback::Unaltered => Route::new(NotFound).call(req),
            DefaultFallback::Layered(inner) => inner.clone().call(req),
        }
    }
}

struct InnerRoutes<T> {
    route_id_to_endpoint: HashMap<RouteId, T>,
    node: Arc<Node>,
}

impl<T> InnerRoutes<T> {
    fn try_insert<'path>(
        &mut self,
        path: &'path str,
        route: T,
    ) -> Result<RouteId, InsertError<'path, T>> {
        if path.ends_with("//") {
            panic!("double slash: {}", path);
        }

        let id = RouteId::next();

        let mut node =
            Arc::try_unwrap(Arc::clone(&self.node)).unwrap_or_else(|node| (*node).clone());

        if let Err(err) = node.insert(path, id) {
            return Err(InsertError { err, path, route });
        }

        self.node = Arc::new(node);

        self.route_id_to_endpoint.insert(id, route);

        Ok(id)
    }

    fn overwrite(&mut self, path: &str, route: T) {
        match self.try_insert(path, route) {
            Ok(_) => {}
            Err(err) => {
                let route = err.route;
                let conflicting_path = match err.err {
                    matchit::InsertError::Conflict { with } => with,
                    _ => unreachable!(),
                };
                let id = self.node.path_to_route_id.get(&*conflicting_path).unwrap();
                self.route_id_to_endpoint.insert(*id, route);
            }
        }
    }

    fn into_iter(self) -> impl Iterator<Item = (Arc<str>, T)> {
        self.route_id_to_endpoint
            .into_iter()
            .map(move |(route_id, route)| {
                let path = &self.node.route_id_to_path[&route_id];
                let path = Arc::clone(path);
                (path, route)
            })
    }

    fn at<'router, 'path>(&'router self, path: &'path str) -> Option<Match<'router, 'path, T>> {
        let matchit::Match { value: id, params } = self.node.at(path).ok()?;

        #[cfg(feature = "matched-path")]
        let matched_path = self.node.route_id_to_path.get(id).unwrap();

        let route = self.route_id_to_endpoint.get(id).unwrap();

        Some(Match {
            params,
            #[cfg(feature = "matched-path")]
            matched_path,
            route,
        })
    }

    fn map_routes<F, K>(self, f: F) -> InnerRoutes<K>
    where
        F: Fn(T) -> K,
    {
        let route_id_to_endpoint = self
            .route_id_to_endpoint
            .into_iter()
            .map(|(id, route)| (id, f(route)))
            .collect();

        InnerRoutes {
            route_id_to_endpoint,
            node: self.node,
        }
    }
}

#[derive(Debug)]
struct InsertError<'a, T> {
    err: matchit::InsertError,
    path: &'a str,
    route: T,
}

impl<'a, T> fmt::Display for InsertError<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid route {:?}: {}", self.path, self.err)
    }
}

struct Match<'router, 'path, T> {
    params: matchit::Params<'router, 'path>,
    #[cfg(feature = "matched-path")]
    matched_path: &'router Arc<str>,
    route: &'router T,
}

impl<T> Default for InnerRoutes<T> {
    fn default() -> Self {
        Self {
            route_id_to_endpoint: Default::default(),
            node: Default::default(),
        }
    }
}

impl<T> Clone for InnerRoutes<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            route_id_to_endpoint: self.route_id_to_endpoint.clone(),
            node: Arc::clone(&self.node),
        }
    }
}

impl<T> fmt::Debug for InnerRoutes<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InnerRoutes")
            .field("route_id_to_endpoint", &self.route_id_to_endpoint)
            .field("node", &self.node)
            .finish()
    }
}

/// Wrapper around `matchit::Router` that supports merging two `Router`s.
#[derive(Clone, Default)]
struct Node {
    inner: matchit::Router<RouteId>,
    route_id_to_path: HashMap<RouteId, Arc<str>>,
    path_to_route_id: HashMap<Arc<str>, RouteId>,
}

impl Node {
    fn insert(
        &mut self,
        path: impl Into<String>,
        val: RouteId,
    ) -> Result<(), matchit::InsertError> {
        let path = path.into();

        self.inner.insert(&path, val)?;

        let shared_path: Arc<str> = path.into();
        self.route_id_to_path.insert(val, shared_path.clone());
        self.path_to_route_id.insert(shared_path, val);

        Ok(())
    }

    fn at<'n, 'p>(
        &'n self,
        path: &'p str,
    ) -> Result<matchit::Match<'n, 'p, &'n RouteId>, MatchError> {
        self.inner.at(path)
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("paths", &self.route_id_to_path)
            .finish()
    }
}

enum Endpoint<B> {
    MethodRouter(MethodRouter<B>),
    Route(Route<B>),
}

impl<B> Clone for Endpoint<B> {
    fn clone(&self) -> Self {
        match self {
            Endpoint::MethodRouter(inner) => Endpoint::MethodRouter(inner.clone()),
            Endpoint::Route(inner) => Endpoint::Route(inner.clone()),
        }
    }
}

impl<B> fmt::Debug for Endpoint<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MethodRouter(inner) => inner.fmt(f),
            Self::Route(inner) => inner.fmt(f),
        }
    }
}

enum DefaultFallback<B> {
    Unaltered,
    Layered(Route<B>),
}

impl<B> Clone for DefaultFallback<B> {
    fn clone(&self) -> Self {
        match self {
            Self::Unaltered => Self::Unaltered,
            Self::Layered(inner) => Self::Layered(inner.clone()),
        }
    }
}

impl<B> fmt::Debug for DefaultFallback<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unaltered => f.debug_tuple("Unaltered").finish(),
            Self::Layered(inner) => f.debug_tuple("Layered").field(inner).finish(),
        }
    }
}

#[test]
#[allow(warnings)]
fn traits() {
    use crate::test_helpers::*;
    assert_send::<Router<()>>();
}
