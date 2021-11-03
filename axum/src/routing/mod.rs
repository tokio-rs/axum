//! Routing between [`Service`]s and handlers.

use self::future::RouterFuture;
use self::not_found::NotFound;
use crate::{
    body::{box_body, Body, BoxBody},
    extract::{
        connect_info::{Connected, IntoMakeServiceWithConnectInfo},
        MatchedPath, OriginalUri,
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
    sync::Arc,
    task::{Context, Poll},
};
use tower::{util::ServiceExt, ServiceBuilder};
use tower_http::map_response_body::MapResponseBodyLayer;
use tower_layer::Layer;
use tower_service::Service;

pub mod future;
pub mod handler_method_routing;
pub mod service_method_routing;

mod into_make_service;
mod method_filter;
mod method_not_allowed;
mod not_found;
mod route;
mod strip_prefix;

#[cfg(tests)]
mod tests;

pub use self::{
    into_make_service::IntoMakeService, method_filter::MethodFilter,
    method_not_allowed::MethodNotAllowed, route::Route,
};

#[doc(no_inline)]
pub use self::handler_method_routing::{
    any, delete, get, head, on, options, patch, post, put, trace, MethodRouter,
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
    fallback: Fallback<B>,
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
    B: Send + 'static,
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
        }
    }

    #[doc = include_str!("../docs/routing/route.md")]
    pub fn route<T>(mut self, path: &str, service: T) -> Self
    where
        T: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible>
            + Clone
            + Send
            + 'static,
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

        if let Err(err) = self.node.insert(path, id) {
            panic!("Invalid route: {}", err);
        }

        self.routes.insert(id, Route::new(service));

        self
    }

    #[doc = include_str!("../docs/routing/nest.md")]
    pub fn nest<T>(mut self, path: &str, svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible>
            + Clone
            + Send
            + 'static,
        T::Future: Send + 'static,
    {
        if path.is_empty() {
            panic!("Invalid route: empty path");
        }

        if path.contains('*') {
            panic!("Invalid route: nested routes cannot contain wildcards (*)");
        }

        let prefix = path;

        match try_downcast::<Router<B>, _>(svc) {
            // if the user is nesting a `Router` we can implement nesting
            // by simplying copying all the routes and adding the prefix in
            // front
            Ok(router) => {
                let Router {
                    mut routes,
                    node,
                    // discard the fallback of the nested router
                    fallback: _,
                } = router;

                for (id, nested_path) in node.paths {
                    let route = routes.remove(&id).unwrap();
                    let full_path = if &*nested_path == "/" {
                        path.to_string()
                    } else {
                        format!("{}{}", path, nested_path)
                    };
                    self = self.route(&full_path, strip_prefix::StripPrefix::new(route, prefix));
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
        } = other;

        if let Err(err) = self.node.merge(node) {
            panic!("Invalid route: {}", err);
        }

        for (id, route) in routes {
            assert!(self.routes.insert(id, route).is_none());
        }

        self.fallback = match (self.fallback, fallback) {
            (Fallback::Default(_), pick @ Fallback::Default(_)) => pick,
            (Fallback::Default(_), pick @ Fallback::Custom(_)) => pick,
            (pick @ Fallback::Custom(_), Fallback::Default(_)) => pick,
            (Fallback::Custom(_), pick @ Fallback::Custom(_)) => pick,
        };

        self
    }

    #[doc = include_str!("../docs/routing/layer.md")]
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
        LayeredResBody: http_body::Body<Data = Bytes> + Send + 'static,
        LayeredResBody::Error: Into<BoxError>,
    {
        let layer = ServiceBuilder::new()
            .layer_fn(Route::new)
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

        let fallback = self.fallback.map(|svc| Layer::layer(&layer, svc));

        Router {
            routes,
            node: self.node,
            fallback,
        }
    }

    #[doc = include_str!("../docs/routing/fallback.md")]
    pub fn fallback<T>(mut self, svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible>
            + Clone
            + Send
            + 'static,
        T::Future: Send + 'static,
    {
        self.fallback = Fallback::Custom(Route::new(svc));
        self
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

        if let Some(matched_path) = self.node.paths.get(&id) {
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
        }

        let params = match_
            .params
            .iter()
            .filter(|(key, _)| !key.starts_with(NEST_TAIL_PARAM))
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<Vec<_>>();

        insert_url_params(&mut req, params);

        let route = self
            .routes
            .get(&id)
            .expect("no route for id. This is a bug in axum. Please file an issue")
            .clone();

        RouterFuture::from_oneshot(route.oneshot(req))
    }
}

impl<B> Service<Request<B>> for Router<B>
where
    B: Send + 'static,
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

        match self.node.at(&path) {
            Ok(match_) => self.call_route(match_, req),
            Err(err) => {
                if err.tsr() {
                    let redirect_to = if let Some(without_tsr) = path.strip_suffix('/') {
                        with_path(req.uri(), without_tsr)
                    } else {
                        with_path(req.uri(), &format!("{}/", path))
                    };
                    let res = Response::builder()
                        .status(StatusCode::MOVED_PERMANENTLY)
                        .header(http::header::LOCATION, redirect_to.to_string())
                        .body(crate::body::empty())
                        .unwrap();
                    RouterFuture::from_response(res)
                } else {
                    match &self.fallback {
                        Fallback::Default(inner) => {
                            RouterFuture::from_oneshot(inner.clone().oneshot(req))
                        }
                        Fallback::Custom(inner) => {
                            RouterFuture::from_oneshot(inner.clone().oneshot(req))
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
    paths: HashMap<RouteId, Arc<str>>,
}

impl Node {
    fn insert(
        &mut self,
        path: impl Into<String>,
        val: RouteId,
    ) -> Result<(), matchit::InsertError> {
        let path = path.into();
        self.inner.insert(&path, val)?;
        self.paths.insert(val, path.into());
        Ok(())
    }

    fn merge(&mut self, other: Node) -> Result<(), matchit::InsertError> {
        for (id, path) in other.paths {
            self.insert(&*path, id)?;
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

enum Fallback<B> {
    Default(Route<B>),
    Custom(Route<B>),
}

impl<B> Clone for Fallback<B> {
    fn clone(&self) -> Self {
        match self {
            Fallback::Default(inner) => Fallback::Default(inner.clone()),
            Fallback::Custom(inner) => Fallback::Custom(inner.clone()),
        }
    }
}

impl<B> fmt::Debug for Fallback<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default(inner) => f.debug_tuple("Default").field(inner).finish(),
            Self::Custom(inner) => f.debug_tuple("Custom").field(inner).finish(),
        }
    }
}

impl<B> Fallback<B> {
    fn map<F, B2>(self, f: F) -> Fallback<B2>
    where
        F: FnOnce(Route<B>) -> Route<B2>,
    {
        match self {
            Fallback::Default(inner) => Fallback::Default(f(inner)),
            Fallback::Custom(inner) => Fallback::Custom(f(inner)),
        }
    }
}

fn try_downcast<T, K>(k: K) -> Result<T, K>
where
    T: 'static,
    K: Send + 'static,
{
    use std::any::Any;

    let k = Box::new(k) as Box<dyn Any + Send + 'static>;
    match k.downcast() {
        Ok(t) => Ok(*t),
        Err(other) => Err(*other.downcast().unwrap()),
    }
}

#[test]
fn traits() {
    use crate::test_helpers::*;
    assert_send::<Router<()>>();
}
