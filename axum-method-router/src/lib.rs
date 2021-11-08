#![warn(
    clippy::all,
    clippy::dbg_macro,
    clippy::todo,
    clippy::empty_enum,
    clippy::enum_glob_use,
    clippy::mem_forget,
    clippy::unused_self,
    clippy::filter_map_next,
    clippy::needless_continue,
    clippy::needless_borrow,
    clippy::match_wildcard_for_single_variants,
    clippy::if_let_mutex,
    clippy::mismatched_target_os,
    clippy::await_holding_lock,
    clippy::match_on_vec_items,
    clippy::imprecise_flops,
    clippy::suboptimal_flops,
    clippy::lossy_float_literal,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::fn_params_excessive_bools,
    clippy::exit,
    clippy::inefficient_to_string,
    clippy::linkedlist,
    clippy::macro_use_imports,
    clippy::option_option,
    clippy::verbose_file_reads,
    clippy::unnested_or_patterns,
    rust_2018_idioms,
    future_incompatible,
    nonstandard_style,
    // TODO(david): enable these lints when stuff is done
    // missing_debug_implementations,
    // missing_docs
)]
#![deny(unreachable_pub, private_in_public)]
#![allow(elided_lifetimes_in_paths, clippy::type_complexity)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(test, allow(clippy::float_cmp))]

use axum::{
    body::{box_body, BoxBody, Bytes},
    handler::Handler,
    http::{Method, Request, Response, StatusCode},
    routing::MethodFilter,
    BoxError,
};
use clone_box_service::CloneBoxService;
use http_body::Empty;
use pin_project_lite::pin_project;
use std::{
    convert::Infallible,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{service_fn, util::Oneshot, ServiceBuilder, ServiceExt};
use tower_http::map_response_body::MapResponseBodyLayer;
use tower_layer::Layer;
use tower_service::Service;

mod clone_box_service;

pub fn get_service<S, B>(svc: S) -> MethodRouter<B, S::Error>
where
    S: Service<Request<B>, Response = Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    MethodRouter::new().get_service(svc)
}

pub fn head_service<S, B>(svc: S) -> MethodRouter<B, S::Error>
where
    S: Service<Request<B>, Response = Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    MethodRouter::new().head_service(svc)
}

pub fn delete_service<S, B>(svc: S) -> MethodRouter<B, S::Error>
where
    S: Service<Request<B>, Response = Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    MethodRouter::new().delete_service(svc)
}

pub fn options_service<S, B>(svc: S) -> MethodRouter<B, S::Error>
where
    S: Service<Request<B>, Response = Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    MethodRouter::new().options_service(svc)
}

pub fn patch_service<S, B>(svc: S) -> MethodRouter<B, S::Error>
where
    S: Service<Request<B>, Response = Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    MethodRouter::new().patch_service(svc)
}

pub fn post_service<S, B>(svc: S) -> MethodRouter<B, S::Error>
where
    S: Service<Request<B>, Response = Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    MethodRouter::new().post_service(svc)
}

pub fn put_service<S, B>(svc: S) -> MethodRouter<B, S::Error>
where
    S: Service<Request<B>, Response = Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    MethodRouter::new().put_service(svc)
}

pub fn trace_service<S, B>(svc: S) -> MethodRouter<B, S::Error>
where
    S: Service<Request<B>, Response = Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    MethodRouter::new().trace_service(svc)
}

pub fn on_service<S, B>(filter: MethodFilter, svc: S) -> MethodRouter<B, S::Error>
where
    S: Service<Request<B>, Response = Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    MethodRouter::new().on_service(filter, svc)
}

pub fn get<H, B, T>(handler: H) -> MethodRouter<B, Infallible>
where
    H: Handler<B, T>,
    B: Send + 'static,
    T: 'static,
{
    MethodRouter::new().get(handler)
}

pub fn head<H, B, T>(handler: H) -> MethodRouter<B, Infallible>
where
    H: Handler<B, T>,
    B: Send + 'static,
    T: 'static,
{
    MethodRouter::new().head(handler)
}

pub fn delete<H, B, T>(handler: H) -> MethodRouter<B, Infallible>
where
    H: Handler<B, T>,
    B: Send + 'static,
    T: 'static,
{
    MethodRouter::new().delete(handler)
}

pub fn options<H, B, T>(handler: H) -> MethodRouter<B, Infallible>
where
    H: Handler<B, T>,
    B: Send + 'static,
    T: 'static,
{
    MethodRouter::new().options(handler)
}

pub fn patch<H, B, T>(handler: H) -> MethodRouter<B, Infallible>
where
    H: Handler<B, T>,
    B: Send + 'static,
    T: 'static,
{
    MethodRouter::new().patch(handler)
}

pub fn post<H, B, T>(handler: H) -> MethodRouter<B, Infallible>
where
    H: Handler<B, T>,
    B: Send + 'static,
    T: 'static,
{
    MethodRouter::new().post(handler)
}

pub fn put<H, B, T>(handler: H) -> MethodRouter<B, Infallible>
where
    H: Handler<B, T>,
    B: Send + 'static,
    T: 'static,
{
    MethodRouter::new().put(handler)
}

pub fn trace<H, B, T>(handler: H) -> MethodRouter<B, Infallible>
where
    H: Handler<B, T>,
    B: Send + 'static,
    T: 'static,
{
    MethodRouter::new().trace(handler)
}

pub fn on<H, B, T>(filter: MethodFilter, handler: H) -> MethodRouter<B, Infallible>
where
    H: Handler<B, T>,
    B: Send + 'static,
    T: 'static,
{
    MethodRouter::new().on(filter, handler)
}

pub struct MethodRouter<B, E> {
    get: Option<MethodRoute<B, E>>,
    head: Option<MethodRoute<B, E>>,
    delete: Option<MethodRoute<B, E>>,
    options: Option<MethodRoute<B, E>>,
    patch: Option<MethodRoute<B, E>>,
    post: Option<MethodRoute<B, E>>,
    put: Option<MethodRoute<B, E>>,
    trace: Option<MethodRoute<B, E>>,
    fallback: MethodRoute<B, E>,
    _request_body: PhantomData<fn() -> (E, B)>,
}

impl<B, E> MethodRouter<B, E> {
    pub fn new() -> Self {
        let fallback = MethodRoute(CloneBoxService::new(service_fn(|_: Request<B>| async {
            let mut response = Response::new(box_body(Empty::new()));
            *response.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
            Ok(response)
        })));

        Self {
            get: None,
            head: None,
            delete: None,
            options: None,
            patch: None,
            post: None,
            put: None,
            trace: None,
            fallback,
            _request_body: PhantomData,
        }
    }
}

impl<B> MethodRouter<B, Infallible>
where
    B: Send + 'static,
{
    pub fn on<H, T>(self, filter: MethodFilter, handler: H) -> Self
    where
        H: Handler<B, T>,
        T: 'static,
    {
        self.on_service(filter, handler.into_service())
    }

    pub fn get<H, T>(self, handler: H) -> Self
    where
        H: Handler<B, T>,
        T: 'static,
    {
        self.get_service(handler.into_service())
    }

    pub fn head<H, T>(self, handler: H) -> Self
    where
        H: Handler<B, T>,
        T: 'static,
    {
        self.head_service(handler.into_service())
    }

    pub fn delete<H, T>(self, handler: H) -> Self
    where
        H: Handler<B, T>,
        T: 'static,
    {
        self.delete_service(handler.into_service())
    }

    pub fn options<H, T>(self, handler: H) -> Self
    where
        H: Handler<B, T>,
        T: 'static,
    {
        self.options_service(handler.into_service())
    }

    pub fn patch<H, T>(self, handler: H) -> Self
    where
        H: Handler<B, T>,
        T: 'static,
    {
        self.patch_service(handler.into_service())
    }

    pub fn post<H, T>(self, handler: H) -> Self
    where
        H: Handler<B, T>,
        T: 'static,
    {
        self.post_service(handler.into_service())
    }

    pub fn put<H, T>(self, handler: H) -> Self
    where
        H: Handler<B, T>,
        T: 'static,
    {
        self.put_service(handler.into_service())
    }

    pub fn trace<H, T>(self, handler: H) -> Self
    where
        H: Handler<B, T>,
        T: 'static,
    {
        self.trace_service(handler.into_service())
    }
}

impl<B, E> MethodRouter<B, E> {
    pub fn on_service<S>(self, filter: MethodFilter, svc: S) -> Self
    where
        S: Service<Request<B>, Response = Response<BoxBody>, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        // written with a pattern match like this to ensure we update all fields
        let Self {
            mut get,
            mut head,
            mut delete,
            mut options,
            mut patch,
            mut post,
            mut put,
            mut trace,
            fallback,
            _request_body: _,
        } = self;
        let svc = Some(MethodRoute(CloneBoxService::new(svc)));
        if filter.contains(MethodFilter::GET) {
            get = svc.clone();
        }
        if filter.contains(MethodFilter::HEAD) {
            head = svc.clone();
        }
        if filter.contains(MethodFilter::DELETE) {
            delete = svc.clone();
        }
        if filter.contains(MethodFilter::OPTIONS) {
            options = svc.clone();
        }
        if filter.contains(MethodFilter::PATCH) {
            patch = svc.clone();
        }
        if filter.contains(MethodFilter::POST) {
            post = svc.clone();
        }
        if filter.contains(MethodFilter::PUT) {
            put = svc.clone();
        }
        if filter.contains(MethodFilter::TRACE) {
            trace = svc;
        }
        Self {
            get,
            head,
            delete,
            options,
            patch,
            post,
            put,
            trace,
            fallback,
            _request_body: PhantomData,
        }
    }

    pub fn get_service<S>(mut self, svc: S) -> Self
    where
        S: Service<Request<B>, Response = Response<BoxBody>, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        self.get = Some(MethodRoute(CloneBoxService::new(svc)));
        self
    }

    pub fn head_service<S>(mut self, svc: S) -> Self
    where
        S: Service<Request<B>, Response = Response<BoxBody>, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        self.head = Some(MethodRoute(CloneBoxService::new(svc)));
        self
    }

    pub fn delete_service<S>(mut self, svc: S) -> Self
    where
        S: Service<Request<B>, Response = Response<BoxBody>, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        self.delete = Some(MethodRoute(CloneBoxService::new(svc)));
        self
    }

    pub fn options_service<S>(mut self, svc: S) -> Self
    where
        S: Service<Request<B>, Response = Response<BoxBody>, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        self.options = Some(MethodRoute(CloneBoxService::new(svc)));
        self
    }

    pub fn patch_service<S>(mut self, svc: S) -> Self
    where
        S: Service<Request<B>, Response = Response<BoxBody>, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        self.patch = Some(MethodRoute(CloneBoxService::new(svc)));
        self
    }

    pub fn post_service<S>(mut self, svc: S) -> Self
    where
        S: Service<Request<B>, Response = Response<BoxBody>, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        self.post = Some(MethodRoute(CloneBoxService::new(svc)));
        self
    }

    pub fn put_service<S>(mut self, svc: S) -> Self
    where
        S: Service<Request<B>, Response = Response<BoxBody>, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        self.put = Some(MethodRoute(CloneBoxService::new(svc)));
        self
    }

    pub fn trace_service<S>(mut self, svc: S) -> Self
    where
        S: Service<Request<B>, Response = Response<BoxBody>, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        self.trace = Some(MethodRoute(CloneBoxService::new(svc)));
        self
    }

    pub fn fallback<S>(mut self, svc: S) -> Self
    where
        S: Service<Request<B>, Response = Response<BoxBody>, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        self.fallback = MethodRoute(CloneBoxService::new(svc));
        self
    }

    pub fn layer<L, NewReqBody, NewResBody, NewError>(
        self,
        layer: L,
    ) -> MethodRouter<NewReqBody, NewError>
    where
        L: Layer<MethodRoute<B, E>>,
        L::Service: Service<Request<NewReqBody>, Response = Response<NewResBody>, Error = NewError>
            + Clone
            + Send
            + 'static,
        <L::Service as Service<Request<NewReqBody>>>::Future: Send + 'static,
        NewResBody: http_body::Body<Data = Bytes> + Send + 'static,
        NewResBody::Error: Into<BoxError>,
    {
        let layer = ServiceBuilder::new()
            .layer_fn(|svc| MethodRoute(CloneBoxService::new(svc)))
            .layer(MapResponseBodyLayer::new(box_body))
            .layer(layer)
            .into_inner();
        let layer_fn = |s| layer.layer(s);

        MethodRouter {
            get: self.get.map(layer_fn),
            head: self.head.map(layer_fn),
            delete: self.delete.map(layer_fn),
            options: self.options.map(layer_fn),
            patch: self.patch.map(layer_fn),
            post: self.post.map(layer_fn),
            put: self.put.map(layer_fn),
            trace: self.trace.map(layer_fn),
            fallback: layer_fn(self.fallback),
            _request_body: PhantomData,
        }
    }

    pub fn route_layer<L, NewResBody>(self, layer: L) -> MethodRouter<B, E>
    where
        L: Layer<MethodRoute<B, E>>,
        L::Service: Service<Request<B>, Response = Response<NewResBody>, Error = E>
            + Clone
            + Send
            + 'static,
        <L::Service as Service<Request<B>>>::Future: Send + 'static,
        NewResBody: http_body::Body<Data = Bytes> + Send + 'static,
        NewResBody::Error: Into<BoxError>,
    {
        let layer = ServiceBuilder::new()
            .layer_fn(|svc| MethodRoute(CloneBoxService::new(svc)))
            .layer(MapResponseBodyLayer::new(box_body))
            .layer(layer)
            .into_inner();
        let layer_fn = |s| layer.layer(s);

        MethodRouter {
            get: self.get.map(layer_fn),
            head: self.head.map(layer_fn),
            delete: self.delete.map(layer_fn),
            options: self.options.map(layer_fn),
            patch: self.patch.map(layer_fn),
            post: self.post.map(layer_fn),
            put: self.put.map(layer_fn),
            trace: self.trace.map(layer_fn),
            fallback: self.fallback,
            _request_body: PhantomData,
        }
    }
}

impl<B, E> Clone for MethodRouter<B, E> {
    fn clone(&self) -> Self {
        Self {
            get: self.get.clone(),
            head: self.head.clone(),
            delete: self.delete.clone(),
            options: self.options.clone(),
            patch: self.patch.clone(),
            post: self.post.clone(),
            put: self.put.clone(),
            trace: self.trace.clone(),
            fallback: self.fallback.clone(),
            _request_body: PhantomData,
        }
    }
}

impl<B, E> Default for MethodRouter<B, E>
where
    B: Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! call {
    (
        $req:expr,
        $method:expr,
        $method_variant:ident,
        $svc:expr
    ) => {
        if $method == Method::$method_variant {
            if let Some(svc) = $svc {
                return MethodRouterFuture::from_oneshot(svc.clone().oneshot($req))
                    .strip_body($method == Method::HEAD);
            }
        }
    };
}

impl<B, E> Service<Request<B>> for MethodRouter<B, E> {
    type Response = Response<BoxBody>;
    type Error = E;
    type Future = MethodRouterFuture<B, E>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let method = req.method().clone();

        // written with a pattern match like this to ensure we call all routes
        let Self {
            get,
            head,
            delete,
            options,
            patch,
            post,
            put,
            trace,
            fallback,
            _request_body: _,
        } = self;

        call!(req, method, GET, get);
        call!(req, method, HEAD, get);
        call!(req, method, HEAD, head);
        call!(req, method, POST, post);
        call!(req, method, OPTIONS, options);
        call!(req, method, PATCH, patch);
        call!(req, method, PUT, put);
        call!(req, method, DELETE, delete);
        call!(req, method, TRACE, trace);

        MethodRouterFuture::from_oneshot(fallback.clone().oneshot(req))
            .strip_body(method == Method::HEAD)
    }
}

pin_project! {
    /// Response future for [`MethodRouter`].
    pub struct MethodRouterFuture<B, E> {
        #[pin]
        inner: Oneshot<MethodRoute<B, E>, Request<B>>,
        strip_body: bool,
    }
}

impl<B, E> MethodRouterFuture<B, E> {
    fn from_oneshot(oneshot: Oneshot<MethodRoute<B, E>, Request<B>>) -> Self {
        Self {
            inner: oneshot,
            strip_body: false,
        }
    }

    fn strip_body(mut self, strip_body: bool) -> Self {
        self.strip_body = strip_body;
        self
    }
}

impl<B, E> Future for MethodRouterFuture<B, E> {
    type Output = Result<Response<BoxBody>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let strip_body = self.strip_body;

        match self.project().inner.poll(cx) {
            Poll::Ready(Ok(res)) => {
                if strip_body {
                    Poll::Ready(Ok(res.map(|_| box_body(Empty::new()))))
                } else {
                    Poll::Ready(Ok(res))
                }
            }
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// How routes are stored inside a [`MethodRouter`].
///
/// You normally shouldn’t need to care about this type. It's used in [`MethodRouter::layer`].
pub struct MethodRoute<B, E>(CloneBoxService<Request<B>, Response<BoxBody>, E>);

impl<B, E> Clone for MethodRoute<B, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<B, E> Service<Request<B>> for MethodRoute<B, E> {
    type Response = Response<BoxBody>;
    type Error = E;
    type Future = MethodRouteFuture<B, E>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        MethodRouteFuture {
            inner: self.0.clone().oneshot(req),
        }
    }
}

pin_project! {
    /// Response future for [`MethodRoute`].
    pub struct MethodRouteFuture<B, E> {
        #[pin]
        inner: Oneshot<CloneBoxService<Request<B>, Response<BoxBody>, E>, Request<B>>,
    }
}

impl<B, E> Future for MethodRouteFuture<B, E> {
    type Output = Result<Response<BoxBody>, E>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use axum::{
        body::Body,
        error_handling::{HandleErrorExt, HandleErrorLayer},
    };
    use tower::{timeout::TimeoutLayer, Service, ServiceExt};
    use tower_http::{auth::RequireAuthorizationLayer, services::fs::ServeDir};

    #[tokio::test]
    async fn method_not_allowed_by_default() {
        let mut svc = MethodRouter::new();
        let (status, body) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn get_handler() {
        let mut svc = MethodRouter::new().get(ok);
        let (status, body) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "ok");
    }

    #[tokio::test]
    async fn get_accepts_head() {
        let mut svc = MethodRouter::new().get(ok);
        let (status, body) = call(Method::HEAD, &mut svc).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn head_takes_precedence_over_get() {
        let mut svc = MethodRouter::new().head(created).get(ok);
        let (status, body) = call(Method::HEAD, &mut svc).await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn layer() {
        let mut svc = MethodRouter::new()
            .get(|| async { std::future::pending::<()>().await })
            .layer(RequireAuthorizationLayer::bearer("password"));

        // method with route
        let (status, _) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // method without route
        let (status, _) = call(Method::DELETE, &mut svc).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn route_layer() {
        let mut svc = MethodRouter::new()
            .get(|| async { std::future::pending::<()>().await })
            .route_layer(RequireAuthorizationLayer::bearer("password"));

        // method with route
        let (status, _) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // method without route
        let (status, _) = call(Method::DELETE, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    }

    #[allow(dead_code)]
    fn buiding_complex_router() {
        let app = axum::Router::new().route(
            "/",
            // use the all the things :bomb:
            get(ok)
                .post(ok)
                .route_layer(RequireAuthorizationLayer::bearer("password"))
                .delete_service(ServeDir::new(".").handle_error(|_| StatusCode::NOT_FOUND))
                .fallback((|| async { StatusCode::NOT_FOUND }).into_service())
                .delete(ok)
                .layer(
                    ServiceBuilder::new()
                        .layer(HandleErrorLayer::new(|_| StatusCode::REQUEST_TIMEOUT))
                        .layer(TimeoutLayer::new(Duration::from_secs(10))),
                ),
        );

        axum::Server::bind(&"0.0.0.0:0".parse().unwrap()).serve(app.into_make_service());
    }

    async fn call<S>(method: Method, svc: &mut S) -> (StatusCode, String)
    where
        S: Service<Request<Body>, Response = Response<BoxBody>, Error = Infallible>,
    {
        let request = Request::builder()
            .uri("/")
            .method(method)
            .body(Body::empty())
            .unwrap();
        let response = svc.ready().await.unwrap().call(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let body = String::from_utf8(hyper::body::to_bytes(body).await.unwrap().to_vec()).unwrap();
        (parts.status, body)
    }

    async fn ok() -> (StatusCode, &'static str) {
        (StatusCode::OK, "ok")
    }

    async fn created() -> (StatusCode, &'static str) {
        (StatusCode::CREATED, "created")
    }
}
