use super::{
    future::RouteFuture, url_params, Endpoint, FallbackRoute, Node, Route, RouteId, Router,
};
use crate::{
    body::{Body, HttpBody},
    response::Response,
};
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
    #[track_caller]
    pub(super) fn new<S>(router: Router<S, B>) -> Self
    where
        S: Clone + Send + Sync + 'static,
    {
        let state = router
            .state
            .expect("Can't turn a `Router` that wants to inherit state into a service");

        let routes = router
            .routes
            .into_iter()
            .map(|(route_id, endpoint)| {
                let route = match endpoint {
                    Endpoint::MethodRouter(method_router) => {
                        Route::new(method_router.with_state(state.clone()))
                    }
                    Endpoint::Route(route) => route,
                };

                (route_id, route)
            })
            .collect();

        Self {
            routes,
            node: router.node,
            fallback: router.fallback.into_fallback_route(&state),
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

struct SuperFallback<B>(SyncWrapper<Route<B>>);
