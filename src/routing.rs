//! Routing between [`Service`]s.

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

/// A filter that matches one or more HTTP method.
#[derive(Debug, Copy, Clone)]
pub enum MethodFilter {
    /// Match any method.
    Any,
    /// Match `CONNECT` requests.
    Connect,
    /// Match `DELETE` requests.
    Delete,
    /// Match `GET` requests.
    Get,
    /// Match `HEAD` requests.
    Head,
    /// Match `OPTIONS` requests.
    Options,
    /// Match `PATCH` requests.
    Patch,
    /// Match `POST` requests.
    Post,
    /// Match `PUT` requests.
    Put,
    /// Match `TRACE` requests.
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

/// A route that sends requests to one of two [`Service`]s depending on the
/// path.
///
/// Created with [`route`](crate::route). See that function for more details.
#[derive(Debug, Clone)]
pub struct Route<S, F> {
    pub(crate) pattern: PathPattern,
    pub(crate) svc: S,
    pub(crate) fallback: F,
}

pub trait RoutingDsl: Sized {
    fn route<T>(self, description: &str, svc: T) -> Route<T, Self>
    where
        T: Service<Request<Body>, Error = Infallible> + Clone,
    {
        Route {
            pattern: PathPattern::new(description),
            svc,
            fallback: self,
        }
    }

    fn nest<T>(self, description: &str, svc: T) -> Nested<T, Self>
    where
        T: Service<Request<Body>, Error = Infallible> + Clone,
    {
        Nested {
            pattern: PathPattern::new(description),
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
    type Future = RouteFuture<S, F>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let f = if let Some(captures) = self.pattern.full_match(req.uri().path()) {
            insert_url_params(&mut req, captures);
            let response_future = self.svc.clone().oneshot(req);
            future::Either::Left(BoxResponseBody(response_future))
        } else {
            let response_future = self.fallback.clone().oneshot(req);
            future::Either::Right(BoxResponseBody(response_future))
        };
        RouteFuture(f)
    }
}

/// The response future for [`Route`].
#[pin_project]
#[derive(Debug)]
pub struct RouteFuture<S, F>(
    #[pin]
    pub(crate)  future::Either<
        BoxResponseBody<Oneshot<S, Request<Body>>>,
        BoxResponseBody<Oneshot<F, Request<Body>>>,
    >,
)
where
    S: Service<Request<Body>>,
    F: Service<Request<Body>>;

impl<S, F, SB, FB> Future for RouteFuture<S, F>
where
    S: Service<Request<Body>, Response = Response<SB>, Error = Infallible>,
    SB: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    SB::Error: Into<BoxError>,
    F: Service<Request<Body>, Response = Response<FB>, Error = Infallible>,
    FB: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    FB::Error: Into<BoxError>,
{
    type Output = Result<Response<BoxBody>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().0.poll(cx)
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

/// A response future that boxes the response body with [`BoxBody`].
#[pin_project]
#[derive(Debug)]
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

/// A [`Service`] that responds with `404 Not Found` to all requests.
///
/// This is used as the bottom service in a router stack. You shouldn't have to
/// use to manually.
#[derive(Debug, Clone, Copy)]
pub struct EmptyRouter;

impl RoutingDsl for EmptyRouter {}

impl Service<Request<Body>> for EmptyRouter {
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = EmptyRouterFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: Request<Body>) -> Self::Future {
        let mut res = Response::new(Body::empty());
        *res.status_mut() = StatusCode::NOT_FOUND;
        EmptyRouterFuture(future::ok(res))
    }
}

opaque_future! {
    /// Response future for [`EmptyRouter`].
    pub type EmptyRouterFuture =
        future::Ready<Result<Response<Body>, Infallible>>;
}

#[derive(Debug, Clone)]
pub(crate) struct PathPattern(Arc<Inner>);

#[derive(Debug)]
struct Inner {
    full_path_regex: Regex,
    capture_group_names: Box<[Bytes]>,
}

impl PathPattern {
    pub(crate) fn new(pattern: &str) -> Self {
        assert!(
            pattern.starts_with('/'),
            "Route description must start with a `/`"
        );

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
    type Future = BoxRouteFuture<B>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request<Body>) -> Self::Future {
        BoxRouteFuture(self.0.clone().oneshot(req))
    }
}

/// The response future for [`BoxRoute`].
#[pin_project]
pub struct BoxRouteFuture<B>(#[pin] InnerFuture<B>);

type InnerFuture<B> = Oneshot<
    Buffer<BoxService<Request<Body>, Response<B>, Infallible>, Request<Body>>,
    Request<Body>,
>;

impl<B> Future for BoxRouteFuture<B>
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

#[derive(Clone, Debug)]
pub struct Layered<S>(S);

impl<S> RoutingDsl for Layered<S> {}

impl<S> Layered<S> {
    pub fn handle_error<F, B, Res>(self, f: F) -> crate::service::HandleError<S, F>
    where
        S: Service<Request<Body>, Response = Response<B>> + Clone,
        F: FnOnce(S::Error) -> Res,
        Res: IntoResponse,
        B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        B::Error: Into<BoxError> + Send + Sync + 'static,
    {
        crate::service::HandleError { inner: self.0, f }
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

/// Nest a group of routes (or a [`Service`]) at some path.
///
/// This allows you to break your application into smaller pieces and compose
/// them together. This will strip the matching prefix from the URL so the
/// nested route will only see the part of URL:
///
/// ```
/// use tower_web::{routing::nest, prelude::*};
///
/// async fn users_get(request: Request<Body>) {
///     // `users_get` doesn't see the whole URL. `nest` will strip the matching
///     // `/api` prefix.
///     assert_eq!(request.uri().path(), "/users");
/// }
///
/// async fn users_post(request: Request<Body>) {}
///
/// async fn careers(request: Request<Body>) {}
///
/// let users_api = route("/users", get(users_get).post(users_post));
///
/// let app = nest("/api", users_api).route("/careers", get(careers));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
/// # };
/// ```
///
/// Take care when using `nest` together with dynamic routes as nesting also
/// captures from the outer routes:
///
/// ```
/// use tower_web::{routing::nest, prelude::*};
///
/// async fn users_get(request: Request<Body>, params: extract::UrlParamsMap) {
///     // Both `version` and `id` were captured even though `users_api` only
///     // explicitly captures `id`.
///     let version = params.get("version");
///     let id = params.get("id");
/// }
///
/// let users_api = route("/users/:id", get(users_get));
///
/// let app = nest("/:version/api", users_api);
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
/// # };
/// ```
///
/// `nest` also accepts any [`Service`]. This can for example be used with
/// [`tower_http::services::ServeDir`] to serve static files from a directory:
///
/// ```
/// use tower_web::{
///     routing::nest, service::get, ServiceExt, prelude::*,
/// };
/// use tower_http::services::ServeDir;
///
/// // Serves files inside the `public` directory at `GET /public/*`
/// let serve_dir_service = ServeDir::new("public")
///     .handle_error(|error: std::io::Error| { /* ... */ });
///
/// let app = nest("/public", get(serve_dir_service));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
/// # };
/// ```
///
/// If necessary you can use [`RoutingDsl::boxed`] to box a group of routes
/// making the type easier to name. This is sometimes useful when working with
/// `nest`.
pub fn nest<S>(description: &str, svc: S) -> Nested<S, EmptyRouter>
where
    S: Service<Request<Body>, Error = Infallible> + Clone,
{
    Nested {
        pattern: PathPattern::new(description),
        svc,
        fallback: EmptyRouter,
    }
}

/// A [`Service`] that has been nested inside a router at some path.
///
/// Created with [`nest`] or [`RoutingDsl::nest`].
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
    type Future = RouteFuture<S, F>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let f = if let Some((prefix, captures)) = self.pattern.prefix_match(req.uri().path()) {
            let without_prefix = strip_prefix(req.uri(), prefix);
            *req.uri_mut() = without_prefix;

            insert_url_params(&mut req, captures);
            let response_future = self.svc.clone().oneshot(req);
            future::Either::Left(BoxResponseBody(response_future))
        } else {
            let response_future = self.fallback.clone().oneshot(req);
            future::Either::Right(BoxResponseBody(response_future))
        };
        RouteFuture(f)
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
