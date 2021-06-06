use crate::{body::BoxBody, response::IntoResponse, ResultExt};
use bytes::Bytes;
use futures_util::{future, ready};
use http::{Method, Request, Response, StatusCode, Uri};
use http_body::Full;
use hyper::Body;
use itertools::Itertools;
use pin_project::pin_project;
use regex::Regex;
use std::{
    borrow::Cow,
    convert::Infallible,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{
    buffer::Buffer,
    util::{BoxService, Oneshot, ServiceExt},
    BoxError, Layer, Service, ServiceBuilder,
};

// ===== DSL =====

#[derive(Debug, Copy, Clone)]
pub enum MethodFilter {
    Any,
    Connect,
    Delete,
    Get,
    Head,
    Options,
    Patch,
    Post,
    Put,
    Trace,
}

impl MethodFilter {
    #[allow(clippy::match_like_matches_macro)]
    pub(crate) fn matches(self, method: &Method) -> bool {
        match (self, method) {
            (MethodFilter::Any, _)
            | (MethodFilter::Connect, &Method::CONNECT)
            | (MethodFilter::Delete, &Method::DELETE)
            | (MethodFilter::Get, &Method::GET)
            | (MethodFilter::Head, &Method::HEAD)
            | (MethodFilter::Options, &Method::OPTIONS)
            | (MethodFilter::Patch, &Method::PATCH)
            | (MethodFilter::Post, &Method::POST)
            | (MethodFilter::Put, &Method::PUT)
            | (MethodFilter::Trace, &Method::TRACE) => true,
            _ => false,
        }
    }
}

#[derive(Clone)]
pub struct Route<S, F> {
    pub(crate) pattern: PathPattern,
    pub(crate) svc: S,
    pub(crate) fallback: F,
}

pub trait RoutingDsl: Sized {
    fn route<T>(self, spec: &str, svc: T) -> Route<T, Self>
    where
        T: Service<Request<Body>, Error = Infallible> + Clone,
    {
        Route {
            pattern: PathPattern::new(spec),
            svc,
            fallback: self,
        }
    }

    fn nest<T>(self, spec: &str, svc: T) -> Nested<T, Self>
    where
        T: Service<Request<Body>, Error = Infallible> + Clone,
    {
        Nested {
            pattern: PathPattern::new(spec),
            svc,
            fallback: self,
        }
    }

    fn boxed<B>(self) -> BoxRoute<B>
    where
        Self: Service<Request<Body>, Response = Response<B>, Error = Infallible> + Send + 'static,
        <Self as Service<Request<Body>>>::Future: Send,
        B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        B::Error: Into<BoxError> + Send + Sync + 'static,
    {
        ServiceBuilder::new()
            .layer_fn(BoxRoute)
            .buffer(1024)
            .layer(BoxService::layer())
            .service(self)
    }

    fn layer<L>(self, layer: L) -> Layered<L::Service>
    where
        L: Layer<Self>,
        L::Service: Service<Request<Body>> + Clone,
    {
        Layered(layer.layer(self))
    }
}

impl<S, F> RoutingDsl for Route<S, F> {}

// ===== Routing service impls =====

impl<S, F, SB, FB> Service<Request<Body>> for Route<S, F>
where
    S: Service<Request<Body>, Response = Response<SB>, Error = Infallible> + Clone,
    SB: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    SB::Error: Into<BoxError>,

    F: Service<Request<Body>, Response = Response<FB>, Error = Infallible> + Clone,
    FB: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    FB::Error: Into<BoxError>,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;

    #[allow(clippy::type_complexity)]
    type Future = future::Either<
        BoxResponseBody<Oneshot<S, Request<Body>>>,
        BoxResponseBody<Oneshot<F, Request<Body>>>,
    >;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        if let Some(captures) = self.pattern.full_match(req.uri().path()) {
            insert_url_params(&mut req, captures);
            let response_future = self.svc.clone().oneshot(req);
            future::Either::Left(BoxResponseBody(response_future))
        } else {
            let response_future = self.fallback.clone().oneshot(req);
            future::Either::Right(BoxResponseBody(response_future))
        }
    }
}

#[derive(Debug)]
pub(crate) struct UrlParams(pub(crate) Vec<(String, String)>);

fn insert_url_params<B>(req: &mut Request<B>, params: Vec<(String, String)>) {
    if let Some(current) = req.extensions_mut().get_mut::<Option<UrlParams>>() {
        let mut current = current.take().unwrap();
        current.0.extend(params);
        req.extensions_mut().insert(Some(current));
    } else {
        req.extensions_mut().insert(Some(UrlParams(params)));
    }
}

#[pin_project]
pub struct BoxResponseBody<F>(#[pin] pub(crate) F);

impl<F, B> Future for BoxResponseBody<F>
where
    F: Future<Output = Result<Response<B>, Infallible>>,
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError>,
{
    type Output = Result<Response<BoxBody>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let response: Response<B> = ready!(self.project().0.poll(cx)).unwrap_infallible();
        let response = response.map(|body| {
            let body = body.map_err(Into::into);
            BoxBody::new(body)
        });
        Poll::Ready(Ok(response))
    }
}

#[derive(Clone, Copy)]
pub struct EmptyRouter;

impl RoutingDsl for EmptyRouter {}

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

// ===== PathPattern =====

#[derive(Debug, Clone)]
pub(crate) struct PathPattern(Arc<Inner>);

#[derive(Debug)]
struct Inner {
    full_path_regex: Regex,
    capture_group_names: Box<[Bytes]>,
}

impl PathPattern {
    pub(crate) fn new(pattern: &str) -> Self {
        let mut capture_group_names = Vec::new();

        let pattern = pattern
            .split('/')
            .map(|part| {
                if let Some(key) = part.strip_prefix(':') {
                    capture_group_names.push(Bytes::copy_from_slice(key.as_bytes()));

                    Cow::Owned(format!("(?P<{}>[^/]*)", key))
                } else {
                    Cow::Borrowed(part)
                }
            })
            .join("/");

        let full_path_regex =
            Regex::new(&format!("^{}", pattern)).expect("invalid regex generated from route");

        Self(Arc::new(Inner {
            full_path_regex,
            capture_group_names: capture_group_names.into(),
        }))
    }

    pub(crate) fn full_match(&self, path: &str) -> Option<Captures> {
        self.do_match(path).and_then(|match_| {
            if match_.full_match {
                Some(match_.captures)
            } else {
                None
            }
        })
    }

    pub(crate) fn prefix_match<'a>(&self, path: &'a str) -> Option<(&'a str, Captures)> {
        self.do_match(path)
            .map(|match_| (match_.matched, match_.captures))
    }

    fn do_match<'a>(&self, path: &'a str) -> Option<Match<'a>> {
        self.0.full_path_regex.captures(path).map(|captures| {
            let matched = captures.get(0).unwrap();
            let full_match = matched.as_str() == path;

            let captures = self
                .0
                .capture_group_names
                .iter()
                .map(|bytes| {
                    std::str::from_utf8(bytes)
                        .expect("bytes were created from str so is valid utf-8")
                })
                .filter_map(|name| captures.name(name).map(|value| (name, value.as_str())))
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect::<Vec<_>>();

            Match {
                captures,
                full_match,
                matched: matched.as_str(),
            }
        })
    }
}

struct Match<'a> {
    captures: Captures,
    // true if regex matched whole path, false if it only matched a prefix
    full_match: bool,
    matched: &'a str,
}

type Captures = Vec<(String, String)>;

// ===== BoxRoute =====

pub struct BoxRoute<B>(Buffer<BoxService<Request<Body>, Response<B>, Infallible>, Request<Body>>);

impl<B> Clone for BoxRoute<B> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<B> RoutingDsl for BoxRoute<B> {}

impl<B> Service<Request<Body>> for BoxRoute<B>
where
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = BoxRouteResponseFuture<B>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request<Body>) -> Self::Future {
        BoxRouteResponseFuture(self.0.clone().oneshot(req))
    }
}

#[pin_project]
pub struct BoxRouteResponseFuture<B>(#[pin] InnerFuture<B>);

type InnerFuture<B> = Oneshot<
    Buffer<BoxService<Request<Body>, Response<B>, Infallible>, Request<Body>>,
    Request<Body>,
>;

impl<B> Future for BoxRouteResponseFuture<B>
where
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Output = Result<Response<BoxBody>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match ready!(self.project().0.poll(cx)) {
            Ok(res) => Poll::Ready(Ok(res.map(BoxBody::new))),
            Err(err) => Poll::Ready(Ok(handle_buffer_error(err))),
        }
    }
}

fn handle_buffer_error(error: BoxError) -> Response<BoxBody> {
    use tower::buffer::error::{Closed, ServiceError};

    let error = match error.downcast::<Closed>() {
        Ok(closed) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(BoxBody::new(Full::from(closed.to_string())))
                .unwrap();
        }
        Err(e) => e,
    };

    let error = match error.downcast::<ServiceError>() {
        Ok(service_error) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(BoxBody::new(Full::from(format!("Service error: {}. This is a bug in tower-web. All inner services should be infallible. Please file an issue", service_error))))
                .unwrap();
        }
        Err(e) => e,
    };

    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(BoxBody::new(Full::from(format!(
            "Uncountered an unknown error: {}. This should never happen. Please file an issue",
            error
        ))))
        .unwrap()
}

// ===== Layered =====

#[derive(Clone, Debug)]
pub struct Layered<S>(S);

impl<S> RoutingDsl for Layered<S> {}

impl<S> Layered<S> {
    pub fn handle_error<F, B, Res>(self, f: F) -> HandleError<S, F>
    where
        S: Service<Request<Body>, Response = Response<B>> + Clone,
        F: FnOnce(S::Error) -> Res,
        Res: IntoResponse,
        B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        B::Error: Into<BoxError> + Send + Sync + 'static,
    {
        HandleError { inner: self.0, f }
    }
}

impl<S, B> Service<Request<Body>> for Layered<S>
where
    S: Service<Request<Body>, Response = Response<B>, Error = Infallible>,
{
    type Response = S::Response;
    type Error = Infallible;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.0.call(req)
    }
}

#[derive(Clone, Copy)]
pub struct HandleError<S, F> {
    inner: S,
    f: F,
}

impl<S, F> RoutingDsl for HandleError<S, F> {}

impl<S, F, B, Res> Service<Request<Body>> for HandleError<S, F>
where
    S: Service<Request<Body>, Response = Response<B>> + Clone,
    F: FnOnce(S::Error) -> Res + Clone,
    Res: IntoResponse,
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = HandleErrorFuture<Oneshot<S, Request<Body>>, F>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        HandleErrorFuture {
            inner: self.inner.clone().oneshot(req),
            f: Some(self.f.clone()),
        }
    }
}

#[pin_project]
pub struct HandleErrorFuture<Fut, F> {
    #[pin]
    inner: Fut,
    f: Option<F>,
}

impl<Fut, F, B, E, Res> Future for HandleErrorFuture<Fut, F>
where
    Fut: Future<Output = Result<Response<B>, E>>,
    F: FnOnce(E) -> Res,
    Res: IntoResponse,
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Output = Result<Response<BoxBody>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match ready!(this.inner.poll(cx)) {
            Ok(res) => Ok(res.map(BoxBody::new)).into(),
            Err(err) => {
                let f = this.f.take().unwrap();
                let res = f(err).into_response();
                Ok(res.map(BoxBody::new)).into()
            }
        }
    }
}

// ===== nesting =====

pub fn nest<S>(spec: &str, svc: S) -> Nested<S, EmptyRouter>
where
    S: Service<Request<Body>, Error = Infallible> + Clone,
{
    Nested {
        pattern: PathPattern::new(spec),
        svc,
        fallback: EmptyRouter,
    }
}

#[derive(Debug, Clone)]
pub struct Nested<S, F> {
    pattern: PathPattern,
    svc: S,
    fallback: F,
}

impl<S, F> RoutingDsl for Nested<S, F> {}

impl<S, F, SB, FB> Service<Request<Body>> for Nested<S, F>
where
    S: Service<Request<Body>, Response = Response<SB>, Error = Infallible> + Clone,
    SB: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    SB::Error: Into<BoxError>,

    F: Service<Request<Body>, Response = Response<FB>, Error = Infallible> + Clone,
    FB: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    FB::Error: Into<BoxError>,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;

    #[allow(clippy::type_complexity)]
    type Future = future::Either<
        BoxResponseBody<Oneshot<S, Request<Body>>>,
        BoxResponseBody<Oneshot<F, Request<Body>>>,
    >;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        if let Some((prefix, captures)) = self.pattern.prefix_match(req.uri().path()) {
            let without_prefix = strip_prefix(req.uri(), prefix);
            *req.uri_mut() = without_prefix;

            insert_url_params(&mut req, captures);
            let response_future = self.svc.clone().oneshot(req);
            future::Either::Left(BoxResponseBody(response_future))
        } else {
            let response_future = self.fallback.clone().oneshot(req);
            future::Either::Right(BoxResponseBody(response_future))
        }
    }
}

fn strip_prefix(uri: &Uri, prefix: &str) -> Uri {
    let path_and_query = if let Some(path_and_query) = uri.path_and_query() {
        let new_path = if let Some(path) = path_and_query.path().strip_prefix(prefix) {
            path
        } else {
            path_and_query.path()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing() {
        assert_match("/", "/");

        assert_match("/foo", "/foo");
        assert_match("/foo/", "/foo/");
        refute_match("/foo", "/foo/");
        refute_match("/foo/", "/foo");

        assert_match("/foo/bar", "/foo/bar");
        refute_match("/foo/bar/", "/foo/bar");
        refute_match("/foo/bar", "/foo/bar/");

        assert_match("/:value", "/foo");
        assert_match("/users/:id", "/users/1");
        assert_match("/users/:id/action", "/users/42/action");
        refute_match("/users/:id/action", "/users/42");
        refute_match("/users/:id", "/users/42/action");
    }

    fn assert_match(route_spec: &'static str, path: &'static str) {
        let route = PathPattern::new(route_spec);
        assert!(
            route.full_match(path).is_some(),
            "`{}` doesn't match `{}`",
            path,
            route_spec
        );
    }

    fn refute_match(route_spec: &'static str, path: &'static str) {
        let route = PathPattern::new(route_spec);
        assert!(
            route.full_match(path).is_none(),
            "`{}` did match `{}` (but shouldn't)",
            path,
            route_spec
        );
    }
}
