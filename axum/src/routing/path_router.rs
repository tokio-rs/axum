use crate::{
    extract::{nested_path::SetNestedPath, Request},
    handler::Handler,
};
use axum_core::response::IntoResponse;
use matchit::MatchError;
use std::{borrow::Cow, collections::HashMap, convert::Infallible, fmt, sync::Arc};
use tower_layer::Layer;
use tower_service::Service;

use super::{
    future::RouteFuture, strip_prefix::StripPrefix, url_params, Endpoint, MethodRouter, Route,
    RouteId, NEST_TAIL_PARAM,
};

pub(super) struct PathRouter<S> {
    routes: Vec<Endpoint<S>>,
    node: Arc<Node>,
    v7_checks: bool,
}

fn validate_path(v7_checks: bool, path: &str) -> Result<(), &'static str> {
    if path.is_empty() {
        return Err("Paths must start with a `/`. Use \"/\" for root routes");
    } else if !path.starts_with('/') {
        return Err("Paths must start with a `/`");
    }

    if v7_checks {
        validate_v07_paths(path)?;
    }

    Ok(())
}

fn validate_v07_paths(path: &str) -> Result<(), &'static str> {
    path.split('/')
        .find_map(|segment| {
            if segment.starts_with(':') {
                Some(Err(
                    "Path segments must not start with `:`. For capture groups, use \
                `{capture}`. If you meant to literally match a segment starting with \
                a colon, call `without_v07_checks` on the router.",
                ))
            } else if segment.starts_with('*') {
                Some(Err(
                    "Path segments must not start with `*`. For wildcard capture, use \
                `{*wildcard}`. If you meant to literally match a segment starting with \
                an asterisk, call `without_v07_checks` on the router.",
                ))
            } else {
                None
            }
        })
        .unwrap_or(Ok(()))
}

impl<S> PathRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub(super) fn without_v07_checks(&mut self) {
        self.v7_checks = false;
    }

    pub(super) fn route(
        &mut self,
        path: &str,
        method_router: MethodRouter<S>,
    ) -> Result<(), Cow<'static, str>> {
        validate_path(self.v7_checks, path)?;

        if let Some((route_id, Endpoint::MethodRouter(prev_method_router))) = self
            .node
            .path_to_route_id
            .get(path)
            .and_then(|route_id| self.routes.get(route_id.0).map(|svc| (*route_id, svc)))
        {
            // if we're adding a new `MethodRouter` to a route that already has one just
            // merge them. This makes `.route("/", get(_)).route("/", post(_))` work
            let service = Endpoint::MethodRouter(
                prev_method_router
                    .clone()
                    .merge_for_path(Some(path), method_router)?,
            );
            self.routes[route_id.0] = service;
        } else {
            let endpoint = Endpoint::MethodRouter(method_router);
            self.new_route(path, endpoint)?;
        }

        Ok(())
    }

    pub(super) fn method_not_allowed_fallback<H, T>(&mut self, handler: &H)
    where
        H: Handler<T, S>,
        T: 'static,
    {
        for endpoint in self.routes.iter_mut() {
            if let Endpoint::MethodRouter(rt) = endpoint {
                *rt = rt.clone().default_fallback(handler.clone());
            }
        }
    }

    pub(super) fn route_service<T>(
        &mut self,
        path: &str,
        service: T,
    ) -> Result<(), Cow<'static, str>>
    where
        T: Service<Request, Error = Infallible> + Clone + Send + Sync + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        self.route_endpoint(path, Endpoint::Route(Route::new(service)))
    }

    pub(super) fn route_endpoint(
        &mut self,
        path: &str,
        endpoint: Endpoint<S>,
    ) -> Result<(), Cow<'static, str>> {
        validate_path(self.v7_checks, path)?;

        self.new_route(path, endpoint)?;

        Ok(())
    }

    fn set_node(&mut self, path: &str, id: RouteId) -> Result<(), String> {
        let node = Arc::make_mut(&mut self.node);

        node.insert(path, id)
            .map_err(|err| format!("Invalid route {path:?}: {err}"))
    }

    fn new_route(&mut self, path: &str, endpoint: Endpoint<S>) -> Result<(), String> {
        let id = RouteId(self.routes.len());
        self.set_node(path, id)?;
        self.routes.push(endpoint);
        Ok(())
    }

    pub(super) fn merge(&mut self, other: Self) -> Result<(), Cow<'static, str>> {
        let Self {
            routes,
            node,
            v7_checks,
        } = other;

        // If either of the two did not allow paths starting with `:` or `*`, do not allow them for the merged router either.
        self.v7_checks |= v7_checks;

        for (id, route) in routes.into_iter().enumerate() {
            let route_id = RouteId(id);
            let path = node
                .route_id_to_path
                .get(&route_id)
                .expect("no path for route id. This is a bug in axum. Please file an issue");

            match route {
                Endpoint::MethodRouter(method_router) => self.route(path, method_router)?,
                Endpoint::Route(route) => self.route_service(path, route)?,
            }
        }

        Ok(())
    }

    pub(super) fn nest(
        &mut self,
        path_to_nest_at: &str,
        router: Self,
    ) -> Result<(), Cow<'static, str>> {
        let prefix = validate_nest_path(self.v7_checks, path_to_nest_at);

        let Self {
            routes,
            node,
            // Ignore the configuration of the nested router
            v7_checks: _,
        } = router;

        for (id, endpoint) in routes.into_iter().enumerate() {
            let route_id = RouteId(id);
            let inner_path = node
                .route_id_to_path
                .get(&route_id)
                .expect("no path for route id. This is a bug in axum. Please file an issue");

            let path = path_for_nested_route(prefix, inner_path);

            let layer = (
                StripPrefix::layer(prefix),
                SetNestedPath::layer(path_to_nest_at),
            );
            match endpoint.layer(layer) {
                Endpoint::MethodRouter(method_router) => {
                    self.route(&path, method_router)?;
                }
                Endpoint::Route(route) => {
                    self.route_endpoint(&path, Endpoint::Route(route))?;
                }
            }
        }

        Ok(())
    }

    pub(super) fn nest_service<T>(
        &mut self,
        path_to_nest_at: &str,
        svc: T,
    ) -> Result<(), Cow<'static, str>>
    where
        T: Service<Request, Error = Infallible> + Clone + Send + Sync + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        let path = validate_nest_path(self.v7_checks, path_to_nest_at);
        let prefix = path;

        let path = if path.ends_with('/') {
            format!("{path}{{*{NEST_TAIL_PARAM}}}")
        } else {
            format!("{path}/{{*{NEST_TAIL_PARAM}}}")
        };

        let layer = (
            StripPrefix::layer(prefix),
            SetNestedPath::layer(path_to_nest_at),
        );
        let endpoint = Endpoint::Route(Route::new(layer.layer(svc)));

        self.route_endpoint(&path, endpoint.clone())?;

        // `/{*rest}` is not matched by `/` so we need to also register a router at the
        // prefix itself. Otherwise if you were to nest at `/foo` then `/foo` itself
        // wouldn't match, which it should
        self.route_endpoint(prefix, endpoint.clone())?;
        if !prefix.ends_with('/') {
            // same goes for `/foo/`, that should also match
            self.route_endpoint(&format!("{prefix}/"), endpoint)?;
        }

        Ok(())
    }

    pub(super) fn layer<L>(self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + Send + Sync + 'static,
        L::Service: Service<Request> + Clone + Send + Sync + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
    {
        let routes = self
            .routes
            .into_iter()
            .map(|endpoint| endpoint.layer(layer.clone()))
            .collect();

        Self {
            routes,
            node: self.node,
            v7_checks: self.v7_checks,
        }
    }

    #[track_caller]
    pub(super) fn route_layer<L>(self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + Send + Sync + 'static,
        L::Service: Service<Request> + Clone + Send + Sync + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
    {
        if self.routes.is_empty() {
            panic!(
                "Adding a route_layer before any routes is a no-op. \
                 Add the routes you want the layer to apply to first."
            );
        }

        let routes = self
            .routes
            .into_iter()
            .map(|endpoint| endpoint.layer(layer.clone()))
            .collect();

        Self {
            routes,
            node: self.node,
            v7_checks: self.v7_checks,
        }
    }

    pub(super) fn has_routes(&self) -> bool {
        !self.routes.is_empty()
    }

    pub(super) fn with_state<S2>(self, state: S) -> PathRouter<S2> {
        let routes = self
            .routes
            .into_iter()
            .map(|endpoint| match endpoint {
                Endpoint::MethodRouter(method_router) => {
                    Endpoint::MethodRouter(method_router.with_state(state.clone()))
                }
                Endpoint::Route(route) => Endpoint::Route(route),
            })
            .collect();

        PathRouter {
            routes,
            node: self.node,
            v7_checks: self.v7_checks,
        }
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn call_with_state(
        &self,
        #[cfg_attr(not(feature = "original-uri"), allow(unused_mut))] mut req: Request,
        state: S,
    ) -> Result<RouteFuture<Infallible>, (Request, S)> {
        #[cfg(feature = "original-uri")]
        {
            use crate::extract::OriginalUri;

            if req.extensions().get::<OriginalUri>().is_none() {
                let original_uri = OriginalUri(req.uri().clone());
                req.extensions_mut().insert(original_uri);
            }
        }

        let (mut parts, body) = req.into_parts();

        match self.node.at(parts.uri.path()) {
            Ok(match_) => {
                let id = *match_.value;

                #[cfg(feature = "matched-path")]
                crate::extract::matched_path::set_matched_path_for_request(
                    id,
                    &self.node.route_id_to_path,
                    &mut parts.extensions,
                );

                url_params::insert_url_params(&mut parts.extensions, &match_.params);

                let endpoint = self
                    .routes
                    .get(id.0)
                    .expect("no route for id. This is a bug in axum. Please file an issue");

                let req = Request::from_parts(parts, body);
                match endpoint {
                    Endpoint::MethodRouter(method_router) => {
                        Ok(method_router.call_with_state(req, state))
                    }
                    Endpoint::Route(route) => Ok(route.clone().call_owned(req)),
                }
            }
            // explicitly handle all variants in case matchit adds
            // new ones we need to handle differently
            Err(MatchError::NotFound) => Err((Request::from_parts(parts, body), state)),
        }
    }
}

impl<S> Default for PathRouter<S> {
    fn default() -> Self {
        Self {
            routes: Default::default(),
            node: Default::default(),
            v7_checks: true,
        }
    }
}

impl<S> fmt::Debug for PathRouter<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PathRouter")
            .field("routes", &self.routes)
            .field("node", &self.node)
            .finish()
    }
}

impl<S> Clone for PathRouter<S> {
    fn clone(&self) -> Self {
        Self {
            routes: self.routes.clone(),
            node: self.node.clone(),
            v7_checks: self.v7_checks,
        }
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

#[track_caller]
fn validate_nest_path(v7_checks: bool, path: &str) -> &str {
    assert!(path.starts_with('/'));
    assert!(path.len() > 1);

    if path.split('/').any(|segment| {
        segment.starts_with("{*") && segment.ends_with('}') && !segment.ends_with("}}")
    }) {
        panic!("Invalid route: nested routes cannot contain wildcards (*)");
    }

    if v7_checks {
        validate_v07_paths(path).unwrap();
    }

    path
}

pub(crate) fn path_for_nested_route<'a>(prefix: &'a str, path: &'a str) -> Cow<'a, str> {
    debug_assert!(prefix.starts_with('/'));
    debug_assert!(path.starts_with('/'));

    if prefix.ends_with('/') {
        format!("{prefix}{}", path.trim_start_matches('/')).into()
    } else if path == "/" {
        prefix.into()
    } else {
        format!("{prefix}{path}").into()
    }
}
