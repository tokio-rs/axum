//! Routing between [`Service`]s and handlers.

use self::{future::RouteFuture, not_found::NotFound};
use crate::{
    body::{boxed, Body, Bytes, HttpBody},
    extract::{connect_info::IntoMakeServiceWithConnectInfo, State},
    handler::{Handler, IntoExtensionService},
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
    marker::PhantomData,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{layer::layer_fn, util::MapRequestLayer, ServiceBuilder};
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

/// The router type for composing handlers and services.
pub struct Router<S, B = Body, R = MissingState> {
    // Invariant: If `R == MissingState` then `state` is `None`
    // If `R == WithState` then state is `Some`
    // `R` cannot have other values
    state: Option<S>,
    routes: HashMap<RouteId, Endpoint<S, B, R>>,
    node: Arc<Node>,
    fallback: Fallback<B>,
    _marker: PhantomData<R>,
}

impl<S, B, R> Clone for Router<S, B, R>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            routes: self.routes.clone(),
            node: Arc::clone(&self.node),
            fallback: self.fallback.clone(),
            _marker: PhantomData,
        }
    }
}

impl<S, B> Default for Router<S, B, MissingState>
where
    B: HttpBody + Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S, B, R> fmt::Debug for Router<S, B, R>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            state,
            routes,
            node,
            fallback,
            _marker,
        } = self;
        f.debug_struct("Router")
            .field("state", &state)
            .field("routes", &routes)
            .field("node", &node)
            .field("fallback", &fallback)
            .finish()
    }
}

pub(crate) const NEST_TAIL_PARAM: &str = "__private__axum_nest_tail_param";
const NEST_TAIL_PARAM_CAPTURE: &str = "/*__private__axum_nest_tail_param";

impl<S, B> Router<S, B, MissingState>
where
    B: HttpBody + Send + 'static,
{
    /// Create a new `Router`.
    ///
    /// Unless you add additional routes this will respond with `404 Not Found` to
    /// all requests.
    pub fn new() -> Self {
        Self {
            state: None,
            routes: Default::default(),
            node: Default::default(),
            fallback: Fallback::Default(Route::new(NotFound)),
            _marker: PhantomData,
        }
    }

    fn state(self, state: S) -> Router<S, B, WithState>
    where
        S: Clone,
    {
        let routes = self
            .routes
            .into_iter()
            .map(|(id, endpoint)| {
                let endpoint = match endpoint {
                    Endpoint::MethodRouter(method_router) => {
                        // the state will be provided later in `<Router as Service>::call`, so its safe to
                        // ignore that it hasn't been provided yet
                        Endpoint::MethodRouter(method_router.change_state_marker())
                    }
                    Endpoint::Route(route) => Endpoint::Route(route),
                };
                (id, endpoint)
            })
            .collect();

        Router {
            state: Some(state),
            routes,
            node: self.node,
            fallback: self.fallback,
            _marker: PhantomData,
        }
    }
}

impl<InnerState, B> Router<InnerState, B, MissingState>
where
    B: HttpBody + Send + 'static,
{
    pub fn map_state<F, OuterState>(self, f: F) -> Router<OuterState, B, MissingState>
    where
        F: Fn(OuterState) -> InnerState + Clone + Send + Sync + 'static,
        OuterState: Clone + Send + Sync + 'static,
        InnerState: Send + Sync + 'static,
    {
        debug_assert!(self.state.is_none());

        let routes = self
            .routes
            .into_iter()
            .map(|(route_id, endpoint)| {
                let endpoint = match endpoint {
                    Endpoint::MethodRouter(method_router) => {
                        // the state will be provided later in `<Router as Service>::call`, so its
                        // safe to ignore that it hasn't been provided yet
                        Endpoint::MethodRouter(method_router.change_state::<OuterState>())
                    }
                    Endpoint::Route(route) => Endpoint::Route(route),
                };
                (route_id, endpoint)
            })
            .collect();

        Router {
            state: None,
            routes,
            node: self.node,
            fallback: self.fallback,
            _marker: PhantomData,
        }
        .layer(MapRequestLayer::new(move |mut req: Request<_>| {
            // TODO(david): this is duplicated in `axum/src/handler/into_extension_service.rs`
            // extract into helper function
            let State(outer_state) = req
                .extensions()
                .get::<State<OuterState>>()
                .unwrap_or_else(|| {
                    panic!(
                        "no state of type `{}` was found. Please file an issue",
                        std::any::type_name::<State<OuterState>>()
                    )
                })
                .clone();

            let inner_state = f(outer_state);

            req.extensions_mut().insert(State(inner_state));

            req
        }))
    }
}

impl<S, B> Router<S, B, WithState>
where
    B: HttpBody + Send + 'static,
    S: Clone,
{
    /// TODO(david): docs
    pub fn with_state(state: S) -> Self {
        Router::new().state(state)
    }
}

impl<B> Router<(), B, WithState>
where
    B: HttpBody + Send + 'static,
{
    /// TODO(david): docs
    pub fn without_state() -> Self {
        Router::with_state(())
    }
}

impl<S, B, R> Router<S, B, R>
where
    B: HttpBody + Send + 'static,
    S: Clone + 'static,
    R: 'static,
{
    #[doc = include_str!("../docs/routing/route.md")]
    pub fn route(
        mut self,
        path: &str,
        // TODO(david): constrain this so it only accepts methods
        // routers containing handlers
        method_router: MethodRouter<S, B, Infallible, MissingState>,
    ) -> Self {
        validate_path_for_route(path);

        let id = RouteId::next();

        match self
            .node
            .path_to_route_id
            .get(path)
            .and_then(|route_id| self.routes.get(route_id).map(|svc| (*route_id, svc)))
        {
            Some((route_id, Endpoint::MethodRouter(prev_method_router))) => {
                // if we're adding a new `MethodRouter` to a route that already has one just
                // merge them. This makes `.route("/", get(_)).route("/", post(_))` work
                let service =
                    Endpoint::MethodRouter(prev_method_router.clone().merge(method_router));

                self.routes.insert(route_id, service);

                self
            }
            Some((_, Endpoint::Route(_))) => {
                // if the endpoint isn't a `MethodRouter` then we have no way of merging things so
                // just panic
                panic!("A route for `{}` with a different HTTP method already exists and the routes could not be merge", path)
            }
            None => {
                // the state will be provided later in `<Router as Service>::call`, so its safe to
                // ignore that it hasn't been provided yet
                let service = Endpoint::MethodRouter(method_router.change_state_marker());

                self.insert_endpoint(path, id, service);

                self
            }
        }
    }

    /// TODO(david): docs
    pub fn route_service<T>(mut self, path: &str, service: T) -> Self
    where
        T: Service<Request<B>, Response = Response, Error = Infallible> + Clone + Send + 'static,
        T::Future: Send + 'static,
    {
        let service = match try_downcast::<Router<S, B, WithState>, _>(service) {
            Ok(_) => {
                panic!("Invalid route: `Router::route` cannot be used with `Router`s. Use `Router::nest` instead")
            }
            Err(svc) => svc,
        };

        validate_path_for_route(path);

        let id = RouteId::next();
        let service = Endpoint::Route(Route::new(service));

        self.insert_endpoint(path, id, service);

        self
    }

    fn insert_endpoint(&mut self, path: &str, id: RouteId, endpoint: Endpoint<S, B, R>) {
        let mut node =
            Arc::try_unwrap(Arc::clone(&self.node)).unwrap_or_else(|node| (*node).clone());
        if let Err(err) = node.insert(path, id) {
            panic!("Invalid route: {}", err);
        }
        self.node = Arc::new(node);

        self.routes.insert(id, endpoint);
    }

    #[doc = include_str!("../docs/routing/nest.md")]
    pub fn nest(mut self, mut path: &str, router: Router<S, B, MissingState>) -> Self {
        validate_path_for_nest(&mut path);

        let prefix = path;

        let Router {
            state,
            mut routes,
            node,
            fallback,
            _marker: _,
        } = router;

        debug_assert!(state.is_none());

        if let Fallback::Custom(_) = fallback {
            panic!("Cannot nest `Router`s that has a fallback");
        }

        for (id, nested_path) in &node.route_id_to_path {
            let route = routes.remove(id).unwrap();
            let full_path: Cow<str> = if &**nested_path == "/" {
                path.into()
            } else if path == "/" {
                (&**nested_path).into()
            } else if let Some(path) = path.strip_suffix('/') {
                format!("{}{}", path, nested_path).into()
            } else {
                format!("{}{}", path, nested_path).into()
            };
            self = match route {
                Endpoint::MethodRouter(method_router) => self.route(
                    &full_path,
                    method_router.layer(layer_fn(|s| StripPrefix::new(s, prefix))),
                ),
                Endpoint::Route(route) => {
                    self.route_service(&full_path, StripPrefix::new(route, prefix))
                }
            };
        }

        debug_assert!(routes.is_empty());

        self
    }

    #[doc = include_str!("../docs/routing/nest_service.md")]
    pub fn nest_service<T>(mut self, mut path: &str, svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response, Error = Infallible> + Clone + Send + 'static,
        T::Future: Send + 'static,
    {
        validate_path_for_nest(&mut path);

        let prefix = path;

        let path = if path.ends_with('/') {
            format!("{}*{}", path, NEST_TAIL_PARAM)
        } else {
            format!("{}/*{}", path, NEST_TAIL_PARAM)
        };

        let svc = strip_prefix::StripPrefix::new(svc, prefix);
        self = self.route_service(&path, svc.clone());

        // `/*rest` is not matched by `/` so we need to also register a router at the
        // prefix itself. Otherwise if you were to nest at `/foo` then `/foo` itself
        // wouldn't match, which it should
        self = self.route_service(prefix, svc.clone());
        // same goes for `/foo/`, that should also match
        self = self.route_service(&format!("{}/", prefix), svc);

        self
    }

    #[doc = include_str!("../docs/routing/merge.md")]
    pub fn merge<R2>(mut self, other: R2) -> Self
    where
        R2: Into<Router<S, B, MissingState>>,
    {
        let Router {
            state,
            routes,
            node,
            fallback,
            _marker: _,
        } = other.into();

        debug_assert!(state.is_none());

        for (id, route) in routes {
            let path = node
                .route_id_to_path
                .get(&id)
                .expect("no path for route id. This is a bug in axum. Please file an issue");
            self = match route {
                Endpoint::MethodRouter(route) => self.route(path, route),
                Endpoint::Route(route) => self.route_service(path, route),
            };
        }

        self.fallback = match (self.fallback, fallback) {
            (Fallback::Default(_), pick @ Fallback::Default(_)) => pick,
            (Fallback::Default(_), pick @ Fallback::Custom(_)) => pick,
            (pick @ Fallback::Custom(_), Fallback::Default(_)) => pick,
            (Fallback::Custom(_), Fallback::Custom(_)) => {
                panic!("Cannot merge two `Router`s that both have a fallback")
            }
        };

        self
    }

    #[doc = include_str!("../docs/routing/layer.md")]
    pub fn layer<L, NewReqBody, NewResBody>(self, layer: L) -> Router<S, NewReqBody, R>
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

        let routes = self
            .routes
            .into_iter()
            .map(|(id, route)| {
                let route = match route {
                    Endpoint::MethodRouter(method_router) => {
                        Endpoint::MethodRouter(method_router.layer(&layer))
                    }
                    Endpoint::Route(route) => Endpoint::Route(Route::new(layer.layer(route))),
                };
                (id, route)
            })
            .collect();

        let fallback = self.fallback.map(|svc| Route::new(layer.layer(svc)));

        Router {
            state: self.state,
            routes,
            node: self.node,
            fallback,
            _marker: self._marker,
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

        let routes = self
            .routes
            .into_iter()
            .map(|(id, route)| {
                let route = match route {
                    Endpoint::MethodRouter(method_router) => {
                        Endpoint::MethodRouter(method_router.layer(&layer))
                    }
                    Endpoint::Route(route) => Endpoint::Route(Route::new(layer.layer(route))),
                };
                (id, route)
            })
            .collect();

        Router {
            state: self.state,
            routes,
            node: self.node,
            fallback: self.fallback,
            _marker: self._marker,
        }
    }

    #[doc = include_str!("../docs/routing/fallback.md")]
    pub fn fallback<H, T>(mut self, handler: H) -> Self
    where
        H: Handler<S, T, B>,
        T: 'static,
        S: Clone + Send + Sync + 'static,
    {
        self.fallback_service(IntoExtensionService::new(handler))
    }

    /// TODO(david): docs
    pub fn fallback_service<T>(mut self, svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response, Error = Infallible> + Clone + Send + 'static,
        T::Future: Send + 'static,
    {
        self.fallback = Fallback::Custom(Route::new(svc));
        self
    }
}

impl<S, B> Router<S, B, WithState>
where
    B: HttpBody + Send + 'static,
    S: Clone + Send + Sync + 'static,
{
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

    #[inline]
    fn call_route(
        &self,
        match_: matchit::Match<&RouteId>,
        mut req: Request<B>,
    ) -> RouteFuture<B, Infallible> {
        let id = *match_.value;

        #[cfg(feature = "matched-path")]
        if let Some(matched_path) = self.node.route_id_to_path.get(&id) {
            use crate::extract::MatchedPath;

            let matched_path = if let Some(previous) = req.extensions_mut().get::<MatchedPath>() {
                // a previous `MatchedPath` might exist if we're inside a nested Router
                let previous = if let Some(previous) =
                    previous.as_str().strip_suffix(NEST_TAIL_PARAM_CAPTURE)
                {
                    previous
                } else {
                    previous.as_str()
                };

                let matched_path = format!("{}{}", previous, matched_path);
                matched_path.into()
            } else {
                Arc::clone(matched_path)
            };
            req.extensions_mut().insert(MatchedPath(matched_path));
        } else {
            #[cfg(debug_assertions)]
            panic!("should always have a matched path for a route id");
        }

        url_params::insert_url_params(req.extensions_mut(), match_.params);

        let mut route = self
            .routes
            .get(&id)
            .expect("no route for id. This is a bug in axum. Please file an issue")
            .clone();

        match &mut route {
            Endpoint::MethodRouter(inner) => inner.call(req),
            Endpoint::Route(inner) => inner.call(req),
        }
    }
}

impl<S, B> Service<Request<B>> for Router<S, B, WithState>
where
    B: HttpBody + Send + 'static,
    S: Clone + Send + Sync + 'static,
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
        {
            use crate::extract::OriginalUri;

            if req.extensions().get::<OriginalUri>().is_none() {
                let original_uri = OriginalUri(req.uri().clone());
                req.extensions_mut().insert(original_uri);
            }
        }

        let path = req.uri().path().to_owned();

        if req.extensions().get::<State<S>>().is_none() {
            // the `unwrap` is safe because `self.state` is always some if `R = WithState`, which it is
            req.extensions_mut()
                .insert(State(self.state.as_ref().unwrap().clone()));
        }

        match self.node.at(&path) {
            Ok(match_) => self.call_route(match_, req),
            Err(
                MatchError::NotFound
                | MatchError::ExtraTrailingSlash
                | MatchError::MissingTrailingSlash,
            ) => match &self.fallback {
                Fallback::Default(inner) => inner.clone().call(req),
                Fallback::Custom(inner) => inner.clone().call(req),
            },
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum MissingState {}

#[derive(Copy, Clone, Debug)]
pub enum WithState {}

fn validate_path_for_route(path: &str) {
    if path.is_empty() {
        panic!("Paths must start with a `/`. Use \"/\" for root routes");
    } else if !path.starts_with('/') {
        panic!("Paths must start with a `/`");
    }
}

fn validate_path_for_nest(path: &mut &str) {
    if path.is_empty() {
        // nesting at `""` and `"/"` should mean the same thing
        *path = "/";
    }

    if path.contains('*') {
        panic!("Invalid route: nested routes cannot contain wildcards (*)");
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

enum Fallback<B, E = Infallible> {
    Default(Route<B, E>),
    Custom(Route<B, E>),
}

impl<B, E> Clone for Fallback<B, E> {
    fn clone(&self) -> Self {
        match self {
            Fallback::Default(inner) => Fallback::Default(inner.clone()),
            Fallback::Custom(inner) => Fallback::Custom(inner.clone()),
        }
    }
}

impl<B, E> fmt::Debug for Fallback<B, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default(inner) => f.debug_tuple("Default").field(inner).finish(),
            Self::Custom(inner) => f.debug_tuple("Custom").field(inner).finish(),
        }
    }
}

impl<B, E> Fallback<B, E> {
    fn map<F, B2, E2>(self, f: F) -> Fallback<B2, E2>
    where
        F: FnOnce(Route<B, E>) -> Route<B2, E2>,
    {
        match self {
            Fallback::Default(inner) => Fallback::Default(f(inner)),
            Fallback::Custom(inner) => Fallback::Custom(f(inner)),
        }
    }
}

enum Endpoint<S, B, R> {
    MethodRouter(MethodRouter<S, B, Infallible, R>),
    Route(Route<B>),
}

impl<S, B, R> Clone for Endpoint<S, B, R>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Endpoint::MethodRouter(inner) => Endpoint::MethodRouter(inner.clone()),
            Endpoint::Route(inner) => Endpoint::Route(inner.clone()),
        }
    }
}

impl<S, B, R> fmt::Debug for Endpoint<S, B, R>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MethodRouter(inner) => inner.fmt(f),
            Self::Route(inner) => inner.fmt(f),
        }
    }
}

#[test]
#[allow(warnings)]
fn traits() {
    use crate::test_helpers::*;
    assert_send::<Router<(), (), WithState>>();
    assert_send::<Router<(), (), MissingState>>();
}
