use super::{
    future::RouteFuture, url_params, FallbackRoute, IntoMakeService, Node, Route, RouteId, Router,
    SuperFallback,
};
use crate::{
    body::{Body, HttpBody},
    response::Response,
};
use axum_core::response::IntoResponse;
use http::Request;
use matchit::MatchError;
use std::{
    collections::HashMap,
    convert::Infallible,
    sync::Arc,
    task::{Context, Poll},
};
use sync_wrapper::SyncWrapper;
use tower::Service;
use tower_layer::Layer;

/// A [`Router`] converted into a [`Service`].
#[derive(Debug)]
pub struct RouterService<B = Body> {
    routes: HashMap<RouteId, Route<B>>,
    node: Arc<Node>,
    fallback: FallbackRoute<B>,
}

impl<B> RouterService<B>
where
    B: HttpBody + Send + 'static,
{
    pub(super) fn new<S>(router: Router<S, B>, state: S) -> Self
    where
        S: Clone + Send + Sync + 'static,
    {
        let fallback = router.fallback.into_fallback_route(&state);

        let routes = router
            .routes
            .into_iter()
            .map(|(route_id, endpoint)| {
                let route = endpoint.into_route(state.clone());
                (route_id, route)
            })
            .collect();

        Self {
            routes,
            node: router.node,
            fallback,
        }
    }

    #[inline]
    fn call_route(
        &self,
        match_: matchit::Match<&RouteId>,
        mut req: Request<B>,
    ) -> RouteFuture<B, Infallible> {
        let id = *match_.value;

        #[cfg(feature = "matched-path")]
        crate::extract::matched_path::set_matched_path_for_request(
            id,
            &self.node.route_id_to_path,
            req.extensions_mut(),
        );

        url_params::insert_url_params(req.extensions_mut(), match_.params);

        let mut route = self
            .routes
            .get(&id)
            .expect("no route for id. This is a bug in axum. Please file an issue")
            .clone();

        route.call(req)
    }

    /// Apply a [`tower::Layer`] to all routes in the router.
    ///
    /// See [`Router::layer`] for more details.
    pub fn layer<L, NewReqBody>(self, layer: L) -> RouterService<NewReqBody>
    where
        L: Layer<Route<B>> + Clone + Send + 'static,
        L::Service: Service<Request<NewReqBody>> + Clone + Send + 'static,
        <L::Service as Service<Request<NewReqBody>>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request<NewReqBody>>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request<NewReqBody>>>::Future: Send + 'static,
        NewReqBody: 'static,
    {
        let routes = self
            .routes
            .into_iter()
            .map(|(id, route)| (id, route.layer(layer.clone())))
            .collect();

        let fallback = self.fallback.layer(layer);

        RouterService {
            routes,
            node: self.node,
            fallback,
        }
    }

    /// Apply a [`tower::Layer`] to the router that will only run if the request matches
    /// a route.
    ///
    /// See [`Router::route_layer`] for more details.
    pub fn route_layer<L>(self, layer: L) -> Self
    where
        L: Layer<Route<B>> + Clone + Send + 'static,
        L::Service: Service<Request<B>> + Clone + Send + 'static,
        <L::Service as Service<Request<B>>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request<B>>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request<B>>>::Future: Send + 'static,
    {
        let routes = self
            .routes
            .into_iter()
            .map(|(id, route)| (id, route.layer(layer.clone())))
            .collect();

        Self {
            routes,
            node: self.node,
            fallback: self.fallback,
        }
    }

    /// Convert the `RouterService` into a [`MakeService`].
    ///
    /// See [`Router::into_make_service`] for more details.
    ///
    /// [`MakeService`]: tower::make::MakeService
    pub fn into_make_service(self) -> IntoMakeService<Self> {
        IntoMakeService::new(self)
    }

    /// Convert the `RouterService` into a [`MakeService`] which stores information
    /// about the incoming connection.
    ///
    /// See [`Router::into_make_service_with_connect_info`] for more details.
    ///
    /// [`MakeService`]: tower::make::MakeService
    #[cfg(feature = "tokio")]
    pub fn into_make_service_with_connect_info<C>(
        self,
    ) -> crate::extract::connect_info::IntoMakeServiceWithConnectInfo<Self, C> {
        crate::extract::connect_info::IntoMakeServiceWithConnectInfo::new(self)
    }
}

impl<B> Clone for RouterService<B> {
    fn clone(&self) -> Self {
        Self {
            routes: self.routes.clone(),
            node: Arc::clone(&self.node),
            fallback: self.fallback.clone(),
        }
    }
}

impl<B> Service<Request<B>> for RouterService<B>
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
        {
            use crate::extract::OriginalUri;

            if req.extensions().get::<OriginalUri>().is_none() {
                let original_uri = OriginalUri(req.uri().clone());
                req.extensions_mut().insert(original_uri);
            }
        }

        let path = req.uri().path().to_owned();

        match self.node.at(&path) {
            Ok(match_) => {
                match &self.fallback {
                    FallbackRoute::Default(_) => {}
                    FallbackRoute::Service(fallback) => {
                        req.extensions_mut()
                            .insert(SuperFallback(SyncWrapper::new(fallback.clone())));
                    }
                }

                self.call_route(match_, req)
            }
            Err(
                MatchError::NotFound
                | MatchError::ExtraTrailingSlash
                | MatchError::MissingTrailingSlash,
            ) => match &mut self.fallback {
                FallbackRoute::Default(fallback) => {
                    if let Some(super_fallback) = req.extensions_mut().remove::<SuperFallback<B>>()
                    {
                        let mut super_fallback = super_fallback.0.into_inner();
                        super_fallback.call(req)
                    } else {
                        fallback.call(req)
                    }
                }
                FallbackRoute::Service(fallback) => fallback.call(req),
            },
        }
    }
}
