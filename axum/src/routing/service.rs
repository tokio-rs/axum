use std::{
    collections::HashMap,
    convert::Infallible,
    sync::Arc,
    task::{Context, Poll},
};

use http::Request;
use matchit::MatchError;
use tower_service::Service;

use super::{future::RouteFuture, url_params, Node, Route, RouteId, Router};
use crate::{
    body::{Body, HttpBody},
    response::Response,
};

const NEST_TAIL_PARAM_CAPTURE: &str = "/*__private__axum_nest_tail_param";

pub struct RouterService<B = Body> {
    routes: HashMap<RouteId, Route<B>>,
    node: Arc<Node>,
    fallback: Route<B>,
}

impl<B> RouterService<B>
where
    B: HttpBody + Send + 'static,
{
    pub(super) fn new<S>(router: Router<S, B>) -> Self {
        let _state = match router.state {
            Some(s) => s,
            None => {
                panic!("You must not call into_service on a Router constructed with inherit_state");
            }
        };

        Self {
            routes: router
                .routes
                .into_iter()
                .map(|(id, endpoint)| (id, endpoint.into_route()))
                .collect(),
            node: router.node,
            fallback: router.fallback.into_route(),
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
        {
            fn set_matched_path(
                id: RouteId,
                route_id_to_path: &HashMap<RouteId, Arc<str>>,
                extensions: &mut http::Extensions,
            ) {
                if let Some(matched_path) = route_id_to_path.get(&id) {
                    use crate::extract::MatchedPath;

                    let matched_path = if let Some(previous) = extensions.get::<MatchedPath>() {
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
                    extensions.insert(MatchedPath(matched_path));
                } else {
                    #[cfg(debug_assertions)]
                    panic!("should always have a matched path for a route id");
                }
            }

            set_matched_path(id, &self.node.route_id_to_path, req.extensions_mut());
        }

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
            node: self.node.clone(),
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
            Ok(match_) => self.call_route(match_, req),
            Err(
                MatchError::NotFound
                | MatchError::ExtraTrailingSlash
                | MatchError::MissingTrailingSlash,
            ) => self.fallback.clone().call(req),
        }
    }
}
