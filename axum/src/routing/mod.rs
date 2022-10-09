//! Routing between [`Service`]s and handlers.

use self::not_found::NotFound;
#[cfg(feature = "tokio")]
use crate::extract::connect_info::IntoMakeServiceWithConnectInfo;
use crate::{
    body::{Body, HttpBody},
    handler::{BoxedHandler, Handler},
    util::try_downcast,
};
use axum_core::response::IntoResponse;
use http::Request;
use matchit::MatchError;
use std::{
    any::{type_name, TypeId},
    collections::HashMap,
    convert::Infallible,
    fmt,
    sync::Arc,
};
use tower::{util::MapResponseLayer, ServiceBuilder};
use tower_layer::Layer;
use tower_service::Service;

pub mod future;
pub mod method_routing;

mod into_make_service;
mod method_filter;
mod not_found;
mod route;
mod strip_prefix;
pub(crate) mod url_params;

mod service;
#[cfg(test)]
mod tests;

pub use self::{
    into_make_service::IntoMakeService, method_filter::MethodFilter, route::Route,
    service::RouterService,
};

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
pub struct Router<S = (), B = Body> {
    state: Option<S>,
    routes: HashMap<RouteId, Endpoint<S, B>>,
    node: Arc<Node>,
    fallback: Fallback<S, B>,
}

impl<S, B> Clone for Router<S, B>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            routes: self.routes.clone(),
            node: Arc::clone(&self.node),
            fallback: self.fallback.clone(),
        }
    }
}

impl<S, B> Default for Router<S, B>
where
    B: HttpBody + Send + 'static,
    S: Default + Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::with_state(S::default())
    }
}

impl<S, B> fmt::Debug for Router<S, B>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Router")
            .field("state", &self.state)
            .field("routes", &self.routes)
            .field("node", &self.node)
            .field("fallback", &self.fallback)
            .finish()
    }
}

pub(crate) const NEST_TAIL_PARAM: &str = "__private__axum_nest_tail_param";
const NEST_TAIL_PARAM_CAPTURE: &str = "/*__private__axum_nest_tail_param";

impl<B> Router<(), B>
where
    B: HttpBody + Send + 'static,
{
    /// Create a new `Router`.
    ///
    /// Unless you add additional routes this will respond with `404 Not Found` to
    /// all requests.
    pub fn new() -> Self {
        Self::with_state(())
    }
}

impl<S, B> Router<S, B>
where
    B: HttpBody + Send + 'static,
    S: Clone + Send + Sync + 'static,
{
    /// Create a new `Router` with the given state.
    ///
    /// See [`State`](crate::extract::State) for more details about accessing state.
    ///
    /// Unless you add additional routes this will respond with `404 Not Found` to
    /// all requests.
    pub fn with_state(state: S) -> Self {
        Self {
            state: Some(state),
            routes: Default::default(),
            node: Default::default(),
            fallback: Fallback::Default(Route::new(NotFound)),
        }
    }

    /// Create a new `Router` that inherits its state from another `Router` that it is merged into
    /// or nested under.
    pub fn inherit_state() -> Self {
        Self {
            state: None,
            routes: Default::default(),
            node: Default::default(),
            fallback: Fallback::Default(Route::new(NotFound)),
        }
    }

    #[doc = include_str!("../docs/routing/route.md")]
    #[track_caller]
    pub fn route(mut self, path: &str, method_router: MethodRouter<S, B>) -> Self {
        #[track_caller]
        fn validate_path(path: &str) {
            if path.is_empty() {
                panic!("Paths must start with a `/`. Use \"/\" for root routes");
            } else if !path.starts_with('/') {
                panic!("Paths must start with a `/`");
            }
        }

        validate_path(path);

        let id = RouteId::next();

        let endpoint = if let Some((route_id, Endpoint::MethodRouter(prev_method_router))) = self
            .node
            .path_to_route_id
            .get(path)
            .and_then(|route_id| self.routes.get(route_id).map(|svc| (*route_id, svc)))
        {
            // if we're adding a new `MethodRouter` to a route that already has one just
            // merge them. This makes `.route("/", get(_)).route("/", post(_))` work
            let service = Endpoint::MethodRouter(
                prev_method_router
                    .clone()
                    .merge_for_path(Some(path), method_router),
            );
            self.routes.insert(route_id, service);
            return self;
        } else {
            Endpoint::MethodRouter(method_router)
        };

        self.set_node(path, id);
        self.routes.insert(id, endpoint);

        self
    }

    #[doc = include_str!("../docs/routing/route_service.md")]
    pub fn route_service<T>(mut self, path: &str, service: T) -> Self
    where
        T: Service<Request<B>, Error = Infallible> + Clone + Send + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        if path.is_empty() {
            panic!("Paths must start with a `/`. Use \"/\" for root routes");
        } else if !path.starts_with('/') {
            panic!("Paths must start with a `/`");
        }

        let service = match try_downcast::<RouterService<B>, _>(service) {
            Ok(_) => {
                panic!(
                    "Invalid route: `Router::route_service` cannot be used with `RouterService`s. \
                     Use `Router::nest` instead"
                );
            }
            Err(svc) => svc,
        };

        let id = RouteId::next();
        let endpoint = Endpoint::Route(Route::new(service));
        self.set_node(path, id);
        self.routes.insert(id, endpoint);

        self
    }

    #[track_caller]
    fn set_node(&mut self, path: &str, id: RouteId) {
        let mut node =
            Arc::try_unwrap(Arc::clone(&self.node)).unwrap_or_else(|node| (*node).clone());
        if let Err(err) = node.insert(path, id) {
            panic!("Invalid route {path:?}: {err}");
        }
        self.node = Arc::new(node);
    }

    #[doc = include_str!("../docs/routing/nest.md")]
    #[track_caller]
    pub fn nest<S2>(self, path: &str, mut router: Router<S2, B>) -> Self
    where
        S2: Clone + Send + Sync + 'static,
    {
        if router.state.is_none() {
            let s = self.state.clone();
            router.state = match try_downcast::<Option<S2>, Option<S>>(s) {
                Ok(state) => state,
                Err(_) => panic!(
                    "can't nest a `Router` that wants to inherit state of type `{}` \
                     into a `Router` with a state type of `{}`",
                    type_name::<S2>(),
                    type_name::<S>(),
                ),
            };
        }

        self.nest_service(path, router.into_service())
    }

    /// Like [`nest`](Self::nest), but accepts an arbitrary `Service`.
    #[track_caller]
    pub fn nest_service<T>(mut self, mut path: &str, svc: T) -> Self
    where
        T: Service<Request<B>, Error = Infallible> + Clone + Send + 'static,
        T::Response: IntoResponse,
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
            format!("{path}*{NEST_TAIL_PARAM}")
        } else {
            format!("{path}/*{NEST_TAIL_PARAM}")
        };

        let svc = strip_prefix::StripPrefix::new(svc, prefix);
        self = self.route_service(&path, svc.clone());

        // `/*rest` is not matched by `/` so we need to also register a router at the
        // prefix itself. Otherwise if you were to nest at `/foo` then `/foo` itself
        // wouldn't match, which it should
        self = self.route_service(prefix, svc.clone());
        if !prefix.ends_with('/') {
            // same goes for `/foo/`, that should also match
            self = self.route_service(&format!("{prefix}/"), svc);
        }

        self
    }

    #[doc = include_str!("../docs/routing/merge.md")]
    #[track_caller]
    pub fn merge<S2, R>(mut self, other: R) -> Self
    where
        R: Into<Router<S2, B>>,
        S2: Clone + Send + Sync + 'static,
    {
        let Router {
            state,
            routes,
            node,
            fallback,
        } = other.into();

        let cast_method_router_closure_slot;
        let (fallback, cast_method_router) = match state {
            // other has its state set
            Some(state) => {
                let fallback = fallback.map_state(&state);
                cast_method_router_closure_slot = move |r: MethodRouter<_, _>| r.map_state(&state);
                let cast_method_router = &cast_method_router_closure_slot
                    as &dyn Fn(MethodRouter<_, _>) -> MethodRouter<_, _>;

                (fallback, cast_method_router)
            }
            // other wants to inherit its state
            None => {
                if TypeId::of::<S>() != TypeId::of::<S2>() {
                    panic!(
                        "can't merge a `Router` that wants to inherit state of type `{}` \
                         into a `Router` with a state type of `{}`",
                        type_name::<S2>(),
                        type_name::<S>(),
                    );
                }

                // With the branch above not taken, we know we can cast S2 to S
                let fallback = fallback.downcast_state::<S>().unwrap();

                fn cast_method_router<S, S2, B>(r: MethodRouter<S2, B>) -> MethodRouter<S, B>
                where
                    B: Send + 'static,
                    S: 'static,
                    S2: Clone + 'static,
                {
                    r.downcast_state().unwrap()
                }

                (fallback, &cast_method_router as _)
            }
        };

        for (id, route) in routes {
            let path = node
                .route_id_to_path
                .get(&id)
                .expect("no path for route id. This is a bug in axum. Please file an issue");
            self = match route {
                Endpoint::MethodRouter(method_router) => {
                    self.route(path, cast_method_router(method_router))
                }
                Endpoint::Route(route) => self.route_service(path, route),
            };
        }

        self.fallback = self
            .fallback
            .merge(fallback)
            .expect("Cannot merge two `Router`s that both have a fallback");

        self
    }

    #[doc = include_str!("../docs/routing/layer.md")]
    pub fn layer<L, NewReqBody: 'static>(self, layer: L) -> Router<S, NewReqBody>
    where
        L: Layer<Route<B>> + Clone + Send + 'static,
        L::Service: Service<Request<NewReqBody>> + Clone + Send + 'static,
        <L::Service as Service<Request<NewReqBody>>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request<NewReqBody>>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request<NewReqBody>>>::Future: Send + 'static,
    {
        let layer = ServiceBuilder::new()
            .map_err(Into::into)
            .layer(MapResponseLayer::new(IntoResponse::into_response))
            .layer(layer)
            .into_inner();

        let routes = self
            .routes
            .into_iter()
            .map(|(id, route)| {
                let route = match route {
                    Endpoint::MethodRouter(method_router) => {
                        Endpoint::MethodRouter(method_router.layer(layer.clone()))
                    }
                    Endpoint::Route(route) => Endpoint::Route(Route::new(layer.layer(route))),
                };
                (id, route)
            })
            .collect();

        let fallback = self.fallback.map(move |svc| Route::new(layer.layer(svc)));

        Router {
            state: self.state,
            routes,
            node: self.node,
            fallback,
        }
    }

    #[doc = include_str!("../docs/routing/route_layer.md")]
    #[track_caller]
    pub fn route_layer<L>(self, layer: L) -> Self
    where
        L: Layer<Route<B>> + Clone + Send + 'static,
        L::Service: Service<Request<B>> + Clone + Send + 'static,
        <L::Service as Service<Request<B>>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request<B>>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request<B>>>::Future: Send + 'static,
    {
        if self.routes.is_empty() {
            panic!(
                "Adding a route_layer before any routes is a no-op. \
                 Add the routes you want the layer to apply to first."
            );
        }

        let layer = ServiceBuilder::new()
            .map_err(Into::into)
            .layer(MapResponseLayer::new(IntoResponse::into_response))
            .layer(layer)
            .into_inner();

        let routes = self
            .routes
            .into_iter()
            .map(|(id, route)| {
                let route = match route {
                    Endpoint::MethodRouter(method_router) => {
                        Endpoint::MethodRouter(method_router.layer(layer.clone()))
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
        }
    }

    #[doc = include_str!("../docs/routing/fallback.md")]
    pub fn fallback<H, T>(mut self, handler: H) -> Self
    where
        H: Handler<T, S, B>,
        T: 'static,
    {
        self.fallback = Fallback::BoxedHandler(BoxedHandler::new(handler));
        self
    }

    /// Add a fallback [`Service`] to the router.
    ///
    /// See [`Router::fallback`] for more details.
    pub fn fallback_service<T>(mut self, svc: T) -> Self
    where
        T: Service<Request<B>, Error = Infallible> + Clone + Send + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        self.fallback = Fallback::Service(Route::new(svc));
        self
    }

    /// Convert this router into a [`RouterService`].
    ///
    /// # Panics
    ///
    /// Panics if the router was constructed with [`Router::inherit_state`].
    #[track_caller]
    pub fn into_service(self) -> RouterService<B> {
        RouterService::new(self)
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
    #[track_caller]
    pub fn into_make_service(self) -> IntoMakeService<RouterService<B>> {
        IntoMakeService::new(self.into_service())
    }

    #[doc = include_str!("../docs/routing/into_make_service_with_connect_info.md")]
    #[cfg(feature = "tokio")]
    pub fn into_make_service_with_connect_info<C>(
        self,
    ) -> IntoMakeServiceWithConnectInfo<RouterService<B>, C> {
        IntoMakeServiceWithConnectInfo::new(self.into_service())
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

enum Fallback<S, B, E = Infallible> {
    Default(Route<B, E>),
    Service(Route<B, E>),
    BoxedHandler(BoxedHandler<S, B, E>),
}

impl<S, B, E> Fallback<S, B, E>
where
    S: Clone,
{
    fn map_state<S2>(self, state: &S) -> Fallback<S2, B, E> {
        match self {
            Self::Default(route) => Fallback::Default(route),
            Self::Service(route) => Fallback::Service(route),
            Self::BoxedHandler(handler) => Fallback::Service(handler.into_route(state.clone())),
        }
    }

    fn downcast_state<S2>(self) -> Option<Fallback<S2, B, E>>
    where
        S: 'static,
        B: 'static,
        E: 'static,
        S2: 'static,
    {
        match self {
            Self::Default(route) => Some(Fallback::Default(route)),
            Self::Service(route) => Some(Fallback::Service(route)),
            Self::BoxedHandler(handler) => {
                try_downcast::<BoxedHandler<S2, B, E>, BoxedHandler<S, B, E>>(handler)
                    .map(Fallback::BoxedHandler)
                    .ok()
            }
        }
    }

    fn merge(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::Default(_), pick @ Self::Default(_)) => Some(pick),
            (Self::Default(_), pick) | (pick, Self::Default(_)) => Some(pick),
            _ => None,
        }
    }

    fn into_route(self, state: &S) -> Route<B, E> {
        match self {
            Self::Default(route) => route,
            Self::Service(route) => route,
            Self::BoxedHandler(handler) => handler.into_route(state.clone()),
        }
    }
}

impl<S, B, E> Clone for Fallback<S, B, E> {
    fn clone(&self) -> Self {
        match self {
            Self::Default(inner) => Self::Default(inner.clone()),
            Self::Service(inner) => Self::Service(inner.clone()),
            Self::BoxedHandler(inner) => Self::BoxedHandler(inner.clone()),
        }
    }
}

impl<S, B, E> fmt::Debug for Fallback<S, B, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default(inner) => f.debug_tuple("Default").field(inner).finish(),
            Self::Service(inner) => f.debug_tuple("Service").field(inner).finish(),
            Self::BoxedHandler(_) => f.debug_tuple("BoxedHandler").finish(),
        }
    }
}

impl<S, B, E> Fallback<S, B, E> {
    fn map<F, B2, E2>(self, f: F) -> Fallback<S, B2, E2>
    where
        S: 'static,
        B: 'static,
        E: 'static,
        F: FnOnce(Route<B, E>) -> Route<B2, E2> + Clone + Send + 'static,
        B2: 'static,
        E2: 'static,
    {
        match self {
            Self::Default(inner) => Fallback::Default(f(inner)),
            Self::Service(inner) => Fallback::Service(f(inner)),
            Self::BoxedHandler(inner) => Fallback::BoxedHandler(inner.map(f)),
        }
    }
}

#[allow(clippy::large_enum_variant)] // This type is only used at init time, probably fine
enum Endpoint<S, B> {
    MethodRouter(MethodRouter<S, B>),
    Route(Route<B>),
}

impl<S, B> Clone for Endpoint<S, B> {
    fn clone(&self) -> Self {
        match self {
            Self::MethodRouter(inner) => Self::MethodRouter(inner.clone()),
            Self::Route(inner) => Self::Route(inner.clone()),
        }
    }
}

impl<S, B> fmt::Debug for Endpoint<S, B> {
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
    assert_send::<Router<(), ()>>();
}
