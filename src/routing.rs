use crate::{
    body::{Body, BoxBody},
    error::Error,
    handler::{Handler, HandlerSvc},
    App, IntoService,
};
use bytes::Bytes;
use futures_util::{future, ready};
use http::{Method, Request, Response, StatusCode};
use pin_project::pin_project;
use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{BoxError, Layer, Service};

#[derive(Clone, Copy)]
pub struct EmptyRouter(pub(crate) ());

impl<R> Service<R> for EmptyRouter {
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: R) -> Self::Future {
        let mut res = Response::new(Body::empty());
        *res.status_mut() = StatusCode::NOT_FOUND;
        future::ok(res)
    }
}

#[derive(Debug, Clone)]
pub struct RouteAt<R> {
    pub(crate) app: App<R>,
    pub(crate) route_spec: Bytes,
}

impl<R> RouteAt<R> {
    pub fn get<F, B, T>(self, handler_fn: F) -> RouteBuilder<Route<HandlerSvc<F, B, T>, R>>
    where
        F: Handler<B, T>,
    {
        self.add_route(handler_fn, Method::GET)
    }

    pub fn get_service<S, B>(self, service: S) -> RouteBuilder<Route<S, R>>
    where
        S: Service<Request<Body>, Response = Response<B>> + Clone,
        S::Error: Into<BoxError>,
    {
        self.add_route_service(service, Method::GET)
    }

    pub fn post<F, B, T>(self, handler_fn: F) -> RouteBuilder<Route<HandlerSvc<F, B, T>, R>>
    where
        F: Handler<B, T>,
    {
        self.add_route(handler_fn, Method::POST)
    }

    pub fn post_service<S, B>(self, service: S) -> RouteBuilder<Route<S, R>>
    where
        S: Service<Request<Body>, Response = Response<B>> + Clone,
        S::Error: Into<BoxError>,
    {
        self.add_route_service(service, Method::POST)
    }

    fn add_route<H, B, T>(
        self,
        handler: H,
        method: Method,
    ) -> RouteBuilder<Route<HandlerSvc<H, B, T>, R>>
    where
        H: Handler<B, T>,
    {
        self.add_route_service(HandlerSvc::new(handler), method)
    }

    fn add_route_service<S>(self, service: S, method: Method) -> RouteBuilder<Route<S, R>> {
        assert!(
            self.route_spec.starts_with(b"/"),
            "route spec must start with a slash (`/`)"
        );

        let new_app = App {
            router: Route {
                service,
                route_spec: RouteSpec::new(method, self.route_spec.clone()),
                fallback: self.app.router,
                handler_ready: false,
                fallback_ready: false,
            },
        };

        RouteBuilder {
            app: new_app,
            route_spec: self.route_spec,
        }
    }
}

pub struct RouteBuilder<R> {
    app: App<R>,
    route_spec: Bytes,
}

impl<R> Clone for RouteBuilder<R>
where
    R: Clone,
{
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            route_spec: self.route_spec.clone(),
        }
    }
}

impl<R> RouteBuilder<R> {
    pub fn at(self, route_spec: &str) -> RouteAt<R> {
        self.app.at(route_spec)
    }

    pub fn get<F, B, T>(self, handler_fn: F) -> RouteBuilder<Route<HandlerSvc<F, B, T>, R>>
    where
        F: Handler<B, T>,
    {
        self.app.at_bytes(self.route_spec).get(handler_fn)
    }

    pub fn get_service<S, B>(self, service: S) -> RouteBuilder<Route<S, R>>
    where
        S: Service<Request<Body>, Response = Response<B>> + Clone,
        S::Error: Into<BoxError>,
    {
        self.app.at_bytes(self.route_spec).get_service(service)
    }

    pub fn post<F, B, T>(self, handler_fn: F) -> RouteBuilder<Route<HandlerSvc<F, B, T>, R>>
    where
        F: Handler<B, T>,
    {
        self.app.at_bytes(self.route_spec).post(handler_fn)
    }

    pub fn post_service<S, B>(self, service: S) -> RouteBuilder<Route<S, R>>
    where
        S: Service<Request<Body>, Response = Response<B>> + Clone,
        S::Error: Into<BoxError>,
    {
        self.app.at_bytes(self.route_spec).post_service(service)
    }

    pub fn into_service(self) -> IntoService<R> {
        IntoService {
            app: self.app,
            poll_ready_error: None,
        }
    }
}

pub struct Route<H, F> {
    service: H,
    route_spec: RouteSpec,
    fallback: F,
    handler_ready: bool,
    fallback_ready: bool,
}

impl<H, F> Clone for Route<H, F>
where
    H: Clone,
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            service: self.service.clone(),
            fallback: self.fallback.clone(),
            route_spec: self.route_spec.clone(),
            // important to reset readiness when cloning
            handler_ready: false,
            fallback_ready: false,
        }
    }
}

#[derive(Clone)]
struct RouteSpec {
    method: Method,
    spec: Bytes,
}

impl RouteSpec {
    fn new(method: Method, spec: impl Into<Bytes>) -> Self {
        Self {
            method,
            spec: spec.into(),
        }
    }
}

impl RouteSpec {
    fn matches<B>(&self, req: &Request<B>) -> Option<Vec<(String, String)>> {
        if req.method() != self.method {
            return None;
        }

        let path = req.uri().path().as_bytes();
        let path_parts = path.split(|b| *b == b'/');

        let spec_parts = self.spec.split(|b| *b == b'/');

        if spec_parts.clone().count() != path_parts.clone().count() {
            return None;
        }

        let mut params = Vec::new();

        spec_parts
            .zip(path_parts)
            .all(|(spec, path)| {
                if let Some(key) = spec.strip_prefix(b":") {
                    let key = std::str::from_utf8(key).unwrap().to_string();
                    if let Ok(value) = std::str::from_utf8(path) {
                        params.push((key, value.to_string()));
                        true
                    } else {
                        false
                    }
                } else {
                    spec == path
                }
            })
            .then(|| params)
    }
}

impl<H, F, HB, FB> Service<Request<Body>> for Route<H, F>
where
    H: Service<Request<Body>, Response = Response<HB>>,
    H::Error: Into<Error>,
    HB: http_body::Body + Send + Sync + 'static,
    HB::Error: Into<BoxError>,

    F: Service<Request<Body>, Response = Response<FB>>,
    F::Error: Into<Error>,
    FB: http_body::Body<Data = HB::Data> + Send + Sync + 'static,
    FB::Error: Into<BoxError>,
{
    type Response = Response<BoxBody<HB::Data, Error>>;
    type Error = Error;
    type Future = future::Either<BoxResponseBody<H::Future>, BoxResponseBody<F::Future>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        loop {
            if !self.handler_ready {
                ready!(self.service.poll_ready(cx)).map_err(Into::into)?;
                self.handler_ready = true;
            }

            if !self.fallback_ready {
                ready!(self.fallback.poll_ready(cx)).map_err(Into::into)?;
                self.fallback_ready = true;
            }

            if self.handler_ready && self.fallback_ready {
                return Poll::Ready(Ok(()));
            }
        }
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        if let Some(params) = self.route_spec.matches(&req) {
            assert!(
                self.handler_ready,
                "handler not ready. Did you forget to call `poll_ready`?"
            );

            self.handler_ready = false;

            req.extensions_mut().insert(Some(UrlParams(params)));

            future::Either::Left(BoxResponseBody(self.service.call(req)))
        } else {
            assert!(
                self.fallback_ready,
                "fallback not ready. Did you forget to call `poll_ready`?"
            );

            self.fallback_ready = false;

            // TODO(david): this leads to each route creating one box body, probably not great
            future::Either::Right(BoxResponseBody(self.fallback.call(req)))
        }
    }
}

pub(crate) struct UrlParams(pub(crate) Vec<(String, String)>);

#[pin_project]
pub struct BoxResponseBody<F>(#[pin] F);

impl<F, B, E> Future for BoxResponseBody<F>
where
    F: Future<Output = Result<Response<B>, E>>,
    E: Into<Error>,
    B: http_body::Body + Send + Sync + 'static,
    B::Error: Into<BoxError>,
{
    type Output = Result<Response<BoxBody<B::Data, Error>>, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let response: Response<B> = ready!(self.project().0.poll(cx)).map_err(Into::into)?;
        let response = response.map(|body| {
            // TODO(david): attempt to downcast this into `Error`
            let body = body.map_err(|err| Error::ResponseBody(err.into()));
            BoxBody::new(body)
        });
        Poll::Ready(Ok(response))
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_routing() {
        assert_match((Method::GET, "/"), (Method::GET, "/"));
        refute_match((Method::GET, "/"), (Method::POST, "/"));
        refute_match((Method::POST, "/"), (Method::GET, "/"));

        assert_match((Method::GET, "/foo"), (Method::GET, "/foo"));
        assert_match((Method::GET, "/foo/"), (Method::GET, "/foo/"));
        refute_match((Method::GET, "/foo"), (Method::GET, "/foo/"));
        refute_match((Method::GET, "/foo/"), (Method::GET, "/foo"));

        assert_match((Method::GET, "/foo/bar"), (Method::GET, "/foo/bar"));
        refute_match((Method::GET, "/foo/bar/"), (Method::GET, "/foo/bar"));
        refute_match((Method::GET, "/foo/bar"), (Method::GET, "/foo/bar/"));

        assert_match((Method::GET, "/:value"), (Method::GET, "/foo"));
        assert_match((Method::GET, "/users/:id"), (Method::GET, "/users/1"));
        assert_match(
            (Method::GET, "/users/:id/action"),
            (Method::GET, "/users/42/action"),
        );
        refute_match(
            (Method::GET, "/users/:id/action"),
            (Method::GET, "/users/42"),
        );
        refute_match(
            (Method::GET, "/users/:id"),
            (Method::GET, "/users/42/action"),
        );
    }

    fn assert_match(route_spec: (Method, &'static str), req_spec: (Method, &'static str)) {
        let route = RouteSpec::new(route_spec.0.clone(), route_spec.1);
        let req = Request::builder()
            .method(req_spec.0.clone())
            .uri(req_spec.1)
            .body(())
            .unwrap();

        assert!(
            route.matches(&req).is_some(),
            "`{} {}` doesn't match `{} {}`",
            req.method(),
            req.uri().path(),
            route.method,
            std::str::from_utf8(&route.spec).unwrap(),
        );
    }

    fn refute_match(route_spec: (Method, &'static str), req_spec: (Method, &'static str)) {
        let route = RouteSpec::new(route_spec.0.clone(), route_spec.1);
        let req = Request::builder()
            .method(req_spec.0.clone())
            .uri(req_spec.1)
            .body(())
            .unwrap();

        assert!(
            route.matches(&req).is_none(),
            "`{} {}` shouldn't match `{} {}`",
            req.method(),
            req.uri().path(),
            route.method,
            std::str::from_utf8(&route.spec).unwrap(),
        );
    }

    fn route(method: Method, uri: &'static str) -> RouteSpec {
        RouteSpec::new(method, uri)
    }

    fn req(method: Method, uri: &str) -> Request<()> {
        Request::builder().uri(uri).method(method).body(()).unwrap()
    }
}
