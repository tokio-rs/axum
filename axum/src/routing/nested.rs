use std::{
    convert::Infallible,
    fmt::Debug,
    task::{Context, Poll},
};

use axum_core::{
    body::Body,
    extract::Request as AxumRequest,
    response::{IntoResponse, Response},
};
use http::Request;
use http_body::Body as HttpBody;
use tower::{Layer, Service};

use super::{route::RouteFuture, Endpoint, Route, Router};

#[derive(Clone)]
pub(crate) enum NestedRootMarker {
    Root,
    Slash,
}

/// This enum controls behavior of which routes will be registered with a nested router.
///
/// We consider here two routes - _root_ and _slash_. For example if we nest a [`Nested`] router
/// under prefix `/prefix`, the two routes are:
/// - `/prefix` is the _root_ route and
/// - `/prefix/` is the _slash_ route.
#[allow(clippy::large_enum_variant)]
pub(super) enum RootNestBehavior<S> {
    /// The slash route is configured on the nested router's `/` and root route is explicitly given
    ExplicitRoot(Endpoint<S>),
    /// The root route is handled with the nested router's `/` and slash route is handled by the
    /// fallback.
    RootOnly,
    /// The slash route is handled with the nested router's `/` and root route is handled by the
    /// fallback.
    SlashOnly,
    /// Both the root and slash routes are handled with the nested router's `/`.
    RootAndSlash,
}

impl<S> Clone for RootNestBehavior<S> {
    fn clone(&self) -> Self {
        match self {
            Self::RootOnly => Self::RootOnly,
            Self::ExplicitRoot(endpoint) => Self::ExplicitRoot(endpoint.clone()),
            Self::RootAndSlash => Self::RootAndSlash,
            Self::SlashOnly => Self::SlashOnly,
        }
    }
}

impl<S> Debug for RootNestBehavior<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootOnly => write!(f, "RootOnly"),
            Self::ExplicitRoot(endpoint) => f.debug_tuple("ExplicitRoot").field(endpoint).finish(),
            Self::RootAndSlash => write!(f, "RootAndSlash"),
            Self::SlashOnly => write!(f, "SlashOnly"),
        }
    }
}

/// The nested router type for composing routers.
///
/// This type can be [`nest`]ed into other [`Router`]s. It can be created by some methods on
/// `Router` such as `route_root`.
///
/// [`nest`]: Router::nest()
/// [`route_root`]: Router::route_root()
#[must_use]
pub struct Nested<S = ()> {
    router: Router<S>,
    root: RootNestBehavior<S>,
}

impl<S> Clone for Nested<S> {
    fn clone(&self) -> Self {
        Self {
            router: self.router.clone(),
            root: self.root.clone(),
        }
    }
}

impl<S> Debug for Nested<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Nested")
            .field("router", &self.router)
            .field("root", &self.root)
            .finish()
    }
}

impl<S> From<Router<S>> for Nested<S> {
    fn from(router: Router<S>) -> Self {
        Self {
            router,
            root: RootNestBehavior::RootOnly,
        }
    }
}

impl<S> Nested<S> {
    pub(super) fn new(router: Router<S>, root: RootNestBehavior<S>) -> Self {
        Self { router, root }
    }

    pub(super) fn into_inner(self) -> (Router<S>, RootNestBehavior<S>) {
        let Self { router, root } = self;
        (router, root)
    }

    /// Apply a [`tower::Layer`] to all routes in the router.
    ///
    /// This can be used to add additional processing to a request for a group
    /// of routes including the root route.
    ///
    /// See [`Router::layer`] for more details.
    pub fn layer<L>(self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + Send + Sync + 'static,
        L::Service: Service<AxumRequest> + Clone + Send + Sync + 'static,
        <L::Service as Service<AxumRequest>>::Response: IntoResponse + 'static,
        <L::Service as Service<AxumRequest>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<AxumRequest>>::Future: Send + 'static,
        S: Clone + Send + Sync + 'static,
    {
        let Self { router, root } = self;

        let router = router.layer(layer.clone());

        let root = match root {
            RootNestBehavior::ExplicitRoot(endpoint) => {
                RootNestBehavior::ExplicitRoot(endpoint.layer(layer))
            }
            root @ (RootNestBehavior::RootOnly
            | RootNestBehavior::RootAndSlash
            | RootNestBehavior::SlashOnly) => root,
        };

        Self { router, root }
    }
}

impl<B> Service<Request<B>> for Nested<()>
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
        let call_fallback = |req: AxumRequest| {
            self.router
                .inner
                .catch_all_fallback
                .clone()
                .call_with_state(req, ())
        };

        let mut req = req.map(Body::new);

        if req.uri().path() == "/" {
            let is_root_path = matches!(
                req.extensions_mut().remove::<NestedRootMarker>(),
                Some(NestedRootMarker::Root)
            );
            match (&self.root, is_root_path) {
                (RootNestBehavior::RootAndSlash, _)
                | (RootNestBehavior::RootOnly, true)
                | (RootNestBehavior::SlashOnly | RootNestBehavior::ExplicitRoot(_), false) => {
                    // Continue, the `/` path will match the correct handler.
                }
                (RootNestBehavior::ExplicitRoot(endpoint), true) => {
                    return match endpoint {
                        Endpoint::MethodRouter(method_router) => {
                            method_router.call_with_state(req, ())
                        }
                        Endpoint::Route(route) => route.clone().call_owned(req),
                    };
                }
                (RootNestBehavior::RootOnly, false) | (RootNestBehavior::SlashOnly, true) => {
                    return call_fallback(req);
                }
            }
        }

        self.router.call_with_state(req, ())
    }
}
