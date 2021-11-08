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
    body::{box_body, BoxBody},
    handler::Handler,
    http::{Method, Request, Response, StatusCode},
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
use tower::{service_fn, util::Oneshot, ServiceExt};
use tower_service::Service;

mod clone_box_service;

type Svc<B> = CloneBoxService<Request<B>, Response<BoxBody>, Infallible>;

pub struct MethodRouter<B> {
    get: Option<Svc<B>>,
    fallback: Svc<B>,
    _request_body: PhantomData<fn() -> B>,
}

impl<B> MethodRouter<B>
where
    B: Send + 'static,
{
    pub fn new() -> Self {
        let fallback = CloneBoxService::new(service_fn(|_: Request<B>| async {
            let mut response = Response::new(box_body(Empty::new()));
            *response.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
            Ok(response)
        }));

        Self {
            get: None,
            fallback,
            _request_body: PhantomData,
        }
    }

    pub fn get<H, T>(mut self, handler: H) -> Self
    where
        H: Handler<B, T>,
        T: 'static,
    {
        self.get = Some(CloneBoxService::new(handler.into_service()));
        self
    }
}

impl<B> Clone for MethodRouter<B> {
    fn clone(&self) -> Self {
        Self {
            get: self.get.clone(),
            fallback: self.fallback.clone(),
            _request_body: PhantomData,
        }
    }
}

impl<B> Default for MethodRouter<B>
where
    B: Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<B> Service<Request<B>> for MethodRouter<B> {
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = MethodRouterFuture<B>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let method = req.method();

        if method == Method::GET {
            if let Some(svc) = &self.get {
                return MethodRouterFuture {
                    inner: svc.clone().oneshot(req),
                };
            }
        }

        MethodRouterFuture {
            inner: self.fallback.clone().oneshot(req),
        }
    }
}

pin_project! {
    /// Response future for [`MethodRouter`].
    pub struct MethodRouterFuture<B> {
        #[pin]
        inner: Oneshot<Svc<B>, Request<B>>,
    }
}

impl<B> Future for MethodRouterFuture<B> {
    type Output = Result<Response<BoxBody>, Infallible>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use tower::{Service, ServiceExt};

    #[tokio::test]
    async fn method_not_allowed_by_default() {
        let mut svc = MethodRouter::new();
        let status = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn get_handler() {
        let mut svc = MethodRouter::new().get(ok);
        let status = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::OK);
    }

    async fn call<S>(method: Method, svc: &mut S) -> StatusCode
    where
        S: Service<Request<Body>, Response = Response<BoxBody>, Error = Infallible>,
    {
        let request = Request::builder()
            .uri("/")
            .method(method)
            .body(Body::empty())
            .unwrap();
        let response = svc.ready().await.unwrap().call(request).await.unwrap();
        response.status()
    }

    async fn ok() -> StatusCode {
        StatusCode::OK
    }
}
