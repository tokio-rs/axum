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

pub struct MethodRouter<B, E> {
    get: Option<MethodRoute<B, E>>,
    head: Option<MethodRoute<B, E>>,
    fallback: MethodRoute<B, E>,
    _request_body: PhantomData<fn() -> (E, B)>,
}

impl<B, E> MethodRouter<B, E> {
    pub fn new() -> Self {
        let fallback = MethodRoute(CloneBoxService::new(service_fn(|_: Request<B>| async {
            let mut response = Response::new(box_body(Empty::new()));
            *response.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
            Ok::<_, E>(response)
        })));

        Self {
            get: None,
            head: None,
            fallback,
            _request_body: PhantomData,
        }
    }

    pub fn get_service<T>(mut self, svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response<BoxBody>, Error = E> + Clone + Send + 'static,
        T::Future: Send + 'static,
    {
        self.get = Some(MethodRoute(CloneBoxService::new(svc)));
        self
    }

    pub fn head_service<T>(mut self, svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response<BoxBody>, Error = E> + Clone + Send + 'static,
        T::Future: Send + 'static,
    {
        self.head = Some(MethodRoute(CloneBoxService::new(svc)));
        self
    }

    pub fn fallback<T>(mut self, svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response<BoxBody>, Error = E> + Clone + Send + 'static,
        T::Future: Send + 'static,
    {
        self.fallback = MethodRoute(CloneBoxService::new(svc));
        self
    }

    pub fn layer<L, NewReqBody, NewError, NewResBody>(
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

        MethodRouter {
            get: self.get.map(|svc| layer.layer(svc)),
            head: self.head.map(|svc| layer.layer(svc)),
            fallback: layer.layer(self.fallback),
            _request_body: PhantomData,
        }
    }
}

impl<B> MethodRouter<B, Infallible>
where
    B: Send + 'static,
{
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
}

impl<B, E> Clone for MethodRouter<B, E> {
    fn clone(&self) -> Self {
        Self {
            get: self.get.clone(),
            head: self.head.clone(),
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

        if method == Method::HEAD {
            if let Some(svc) = &self.head {
                return MethodRouterFuture::from_oneshot(svc.clone().oneshot(req)).strip_body(true);
            }
        }

        if method == Method::GET || method == Method::HEAD {
            if let Some(svc) = &self.get {
                return MethodRouterFuture::from_oneshot(svc.clone().oneshot(req))
                    .strip_body(method == Method::HEAD);
            }
        }

        MethodRouterFuture::from_oneshot(self.fallback.clone().oneshot(req))
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

    #[inline]
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

    fn call(&mut self, req: Request<B>) -> Self::Future {
        MethodRouteFuture {
            inner: self.0.clone().oneshot(req),
        }
    }
}

pin_project! {
    /// Response future for [`MethodRouter`].
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
    use axum::{body::Body, error_handling::HandleErrorLayer};
    use tower::{timeout::TimeoutLayer, Service, ServiceBuilder, ServiceExt};

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
            .fallback((|| async { std::future::pending::<()>().await }).into_service())
            .layer(
                ServiceBuilder::new()
                    .layer(HandleErrorLayer::new(|_| StatusCode::REQUEST_TIMEOUT))
                    .layer(TimeoutLayer::new(Duration::from_millis(10))),
            );

        // method with route
        let (status, _) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::REQUEST_TIMEOUT);

        // method without route
        let (status, _) = call(Method::DELETE, &mut svc).await;
        assert_eq!(status, StatusCode::REQUEST_TIMEOUT);
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
