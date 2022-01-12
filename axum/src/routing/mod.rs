//! Routing between [`Service`]s and handlers.

use self::{future::RouterFuture, not_found::NotFound};
use crate::{
    body::{boxed, Body, Bytes, HttpBody},
    extract::{
        connect_info::{Connected, IntoMakeServiceWithConnectInfo},
        MatchedPath, OriginalUri,
    },
    response::IntoResponse,
    response::Redirect,
    response::Response,
    routing::strip_prefix::StripPrefix,
    util::{try_downcast, ByteStr, PercentDecodedByteStr},
    BoxError,
};
use http::{Request, Uri};
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
#[derive(Debug)]
pub struct Router<B = Body> {
    routes: HashMap<RouteId, Endpoint<B>>,
    node: Node,
    fallback: Fallback<B>,
    nested_at_root: bool,
}

impl<B> Clone for Router<B> {
    fn clone(&self) -> Self {
        Self {
            routes: self.routes.clone(),
            node: self.node.clone(),
            fallback: self.fallback.clone(),
            nested_at_root: self.nested_at_root,
        }
    }
}

impl<B> Default for Router<B>
where
    B: Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) const NEST_TAIL_PARAM: &str = "axum_nest";
const NEST_TAIL_PARAM_CAPTURE: &str = "/*axum_nest";

impl<B> Router<B>
where
    B: Send + 'static,
{
    /// Create a new `Router`.
    ///
    /// Unless you add additional routes this will respond with `404 Not Found` to
    /// all requests.
    pub fn new() -> Self {
        Self {
            routes: Default::default(),
            node: Default::default(),
            fallback: Fallback::Default(Route::new(NotFound)),
            nested_at_root: false,
        }
    }

    #[doc = include_str!("../docs/routing/route.md")]
    pub fn route<T>(mut self, path: &str, service: T) -> Self
    where
        T: Service<Request<B>, Response = Response, Error = Infallible> + Clone + Send + 'static,
        T::Future: Send + 'static,
    {
        if path.is_empty() {
            panic!("Invalid route: empty path");
        }

        let service = match try_downcast::<Router<B>, _>(service) {
            Ok(_) => {
                panic!("Invalid route: `Router::route` cannot be used with `Router`s. Use `Router::nest` instead")
            }
            Err(svc) => svc,
        };

        let id = RouteId::next();

        let service = match try_downcast::<MethodRouter<B, Infallible>, _>(service) {
            Ok(method_router) => {
                if let Some((route_id, Endpoint::MethodRouter(prev_method_router))) = self
                    .node
                    .path_to_route_id
                    .get(path)
                    .and_then(|route_id| self.routes.get(route_id).map(|svc| (*route_id, svc)))
                {
                    // if we're adding a new `MethodRouter` to a route that already has one just
                    // merge them. This makes `.route("/", get(_)).route("/", post(_))` work
                    let service =
                        Endpoint::MethodRouter(prev_method_router.clone().merge(method_router));
                    self.routes.insert(route_id, service);
                    return self;
                } else {
                    Endpoint::MethodRouter(method_router)
                }
            }
            Err(service) => Endpoint::Route(Route::new(service)),
        };

        if let Err(err) = self.node.insert(path, id) {
            self.panic_on_matchit_error(err);
        }

        self.routes.insert(id, service);

        self
    }

    #[doc = include_str!("../docs/routing/nest.md")]
    pub fn nest<T>(mut self, mut path: &str, svc: T) -> Self
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

        if path == "/" {
            self.nested_at_root = true;
        }

        match try_downcast::<Router<B>, _>(svc) {
            // if the user is nesting a `Router` we can implement nesting
            // by simplying copying all the routes and adding the prefix in
            // front
            Ok(router) => {
                let Router {
                    mut routes,
                    node,
                    fallback,
                    // nesting a router that has something nested at root
                    // doesn't mean something is nested at root in _this_ router
                    // thus we don't need to propagate that
                    nested_at_root: _,
                } = router;

                if let Fallback::Custom(_) = fallback {
                    panic!("Cannot nest `Router`s that has a fallback");
                }

                for (id, nested_path) in node.route_id_to_path {
                    let route = routes.remove(&id).unwrap();
                    let full_path: Cow<str> = if &*nested_path == "/" {
                        path.into()
                    } else if path == "/" {
                        (&*nested_path).into()
                    } else {
                        format!("{}{}", path, nested_path).into()
                    };
                    self = match route {
                        Endpoint::MethodRouter(method_router) => self.route(
                            &full_path,
                            method_router.layer(layer_fn(|s| StripPrefix::new(s, prefix))),
                        ),
                        Endpoint::Route(route) => {
                            self.route(&full_path, StripPrefix::new(route, prefix))
                        }
                    };
                }

                debug_assert!(routes.is_empty());
            }
            // otherwise we add a wildcard route to the service
            Err(svc) => {
                let path = if path == "/" {
                    format!("/*{}", NEST_TAIL_PARAM)
                } else {
                    format!("{}/*{}", path, NEST_TAIL_PARAM)
                };

                self = self.route(&path, strip_prefix::StripPrefix::new(svc, prefix));
            }
        }

        self
    }

    #[doc = include_str!("../docs/routing/merge.md")]
    pub fn merge(mut self, other: Router<B>) -> Self {
        let Router {
            routes,
            node,
            fallback,
            nested_at_root,
        } = other;

        for (id, route) in routes {
            let path = node
                .route_id_to_path
                .get(&id)
                .expect("no path for route id. This is a bug in axum. Please file an issue");
            self = match route {
                Endpoint::MethodRouter(route) => self.route(path, route),
                Endpoint::Route(route) => self.route(path, route),
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

        self.nested_at_root = self.nested_at_root || nested_at_root;

        self
    }

    #[doc = include_str!("../docs/routing/layer.md")]
    pub fn layer<L, NewReqBody, NewResBody>(self, layer: L) -> Router<NewReqBody>
    where
        L: Layer<Route<B>>,
        L::Service: Service<Request<NewReqBody>, Response = Response<NewResBody>, Error = Infallible>
            + Clone
            + Send
            + 'static,
        <L::Service as Service<Request<NewReqBody>>>::Future: Send + 'static,
        NewResBody: HttpBody<Data = Bytes> + Send + 'static,
        NewResBody::Error: Into<BoxError>,
    {
        let layer = ServiceBuilder::new()
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
            routes,
            node: self.node,
            fallback,
            nested_at_root: self.nested_at_root,
        }
    }

    #[doc = include_str!("../docs/routing/route_layer.md")]
    pub fn route_layer<L, NewResBody>(self, layer: L) -> Self
    where
        L: Layer<Route<B>>,
        L::Service: Service<Request<B>, Response = Response<NewResBody>, Error = Infallible>
            + Clone
            + Send
            + 'static,
        <L::Service as Service<Request<B>>>::Future: Send + 'static,
        NewResBody: HttpBody<Data = Bytes> + Send + 'static,
        NewResBody::Error: Into<BoxError>,
    {
        let layer = ServiceBuilder::new()
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
            routes,
            node: self.node,
            fallback: self.fallback,
            nested_at_root: self.nested_at_root,
        }
    }

    #[doc = include_str!("../docs/routing/fallback.md")]
    pub fn fallback<T>(mut self, svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response, Error = Infallible> + Clone + Send + 'static,
        T::Future: Send + 'static,
    {
        self.fallback = Fallback::Custom(Route::new(svc));
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
    pub fn into_make_service_with_connect_info<C, Target>(
        self,
    ) -> IntoMakeServiceWithConnectInfo<Self, C>
    where
        C: Connected<Target>,
    {
        IntoMakeServiceWithConnectInfo::new(self)
    }

    #[inline]
    fn call_route(&self, match_: matchit::Match<&RouteId>, mut req: Request<B>) -> RouterFuture<B> {
        let id = *match_.value;
        req.extensions_mut().insert(id);

        if let Some(matched_path) = self.node.route_id_to_path.get(&id) {
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

        let params = match_
            .params
            .iter()
            .filter(|(key, _)| !key.starts_with(NEST_TAIL_PARAM))
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect::<Vec<_>>();

        insert_url_params(&mut req, params);

        let mut route = self
            .routes
            .get(&id)
            .expect("no route for id. This is a bug in axum. Please file an issue")
            .clone();

        let future = match &mut route {
            Endpoint::MethodRouter(inner) => inner.call(req),
            Endpoint::Route(inner) => inner.call(req),
        };
        RouterFuture::from_future(future)
    }

    fn panic_on_matchit_error(&self, err: matchit::InsertError) {
        if self.nested_at_root {
            panic!(
                "Invalid route: {}. Note that `nest(\"/\", _)` conflicts with all routes. Use `Router::fallback` instead",
                err,
            );
        } else {
            panic!("Invalid route: {}", err);
        }
    }
}

impl<B> Service<Request<B>> for Router<B>
where
    B: Send + 'static,
{
    type Response = Response;
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

        let path = req.uri().path().to_owned();

        match self.node.at(&path) {
            Ok(match_) => self.call_route(match_, req),
            Err(err) => {
                if err.tsr() {
                    let redirect_to = if let Some(without_tsr) = path.strip_suffix('/') {
                        with_path(req.uri(), without_tsr)
                    } else {
                        with_path(req.uri(), &format!("{}/", path))
                    };
                    let res = Redirect::permanent(redirect_to);
                    RouterFuture::from_response(res.into_response())
                } else {
                    match &self.fallback {
                        Fallback::Default(inner) => {
                            RouterFuture::from_future(inner.clone().call(req))
                        }
                        Fallback::Custom(inner) => {
                            RouterFuture::from_future(inner.clone().call(req))
                        }
                    }
                }
            }
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

/// Wrapper around `matchit::Node` that supports merging two `Node`s.
#[derive(Clone, Default)]
struct Node {
    inner: matchit::Node<RouteId>,
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
    ) -> Result<matchit::Match<'n, 'p, &'n RouteId>, matchit::MatchError> {
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

#[test]
fn traits() {
    use crate::test_helpers::*;
    assert_send::<Router<()>>();
}
