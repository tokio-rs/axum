//! Error handling layer for axum that supports extractors and async functions.
//!
//! This crate provides [`HandleErrorLayer`] which works similarly to
//! [`axum::error_handling::HandleErrorLayer`] except that it supports
//! extractors and async functions:
//!
//! ```rust
//! use axum::{
//!     Router,
//!     BoxError,
//!     response::IntoResponse,
//!     http::{StatusCode, Method, Uri},
//!     routing::get,
//! };
//! use tower::{ServiceBuilder, timeout::error::Elapsed};
//! use std::time::Duration;
//! use axum_handle_error_extract::HandleErrorLayer;
//!
//! let app = Router::new()
//!     .route("/", get(|| async {}))
//!     .layer(
//!         ServiceBuilder::new()
//!             // timeouts produces errors, so we handle those with `handle_error`
//!             .layer(HandleErrorLayer::new(handle_error))
//!             .timeout(Duration::from_secs(10))
//!     );
//!
//! // our handler take can 0 to 16 extractors and the final argument must
//! // always be the error produced by the middleware
//! async fn handle_error(
//!     method: Method,
//!     uri: Uri,
//!     error: BoxError,
//! ) -> impl IntoResponse {
//!     if error.is::<Elapsed>() {
//!         (
//!             StatusCode::REQUEST_TIMEOUT,
//!             format!("{} {} took too long", method, uri),
//!         )
//!     } else {
//!         (
//!             StatusCode::INTERNAL_SERVER_ERROR,
//!             format!("{} {} failed: {}", method, uri, error),
//!         )
//!     }
//! }
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! Not running any extractors is also supported:
//!
//! ```rust
//! use axum::{
//!     Router,
//!     BoxError,
//!     response::IntoResponse,
//!     http::StatusCode,
//!     routing::get,
//! };
//! use tower::{ServiceBuilder, timeout::error::Elapsed};
//! use std::time::Duration;
//! use axum_handle_error_extract::HandleErrorLayer;
//!
//! let app = Router::new()
//!     .route("/", get(|| async {}))
//!     .layer(
//!         ServiceBuilder::new()
//!             .layer(HandleErrorLayer::new(handle_error))
//!             .timeout(Duration::from_secs(10))
//!     );
//!
//! // this function just takes the error
//! async fn handle_error(error: BoxError) -> impl IntoResponse {
//!     if error.is::<Elapsed>() {
//!         (
//!             StatusCode::REQUEST_TIMEOUT,
//!             "Request timeout".to_string(),
//!         )
//!     } else {
//!         (
//!             StatusCode::INTERNAL_SERVER_ERROR,
//!             format!("Unhandled internal error: {}", error),
//!         )
//!     }
//! }
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! See [`axum::error_handling`] for more details on axum's error handling model and
//! [`axum::extract`] for more details on extractors.
//!
//! # The future
//!
//! In axum 0.4 this will replace the current [`axum::error_handling::HandleErrorLayer`].

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
    missing_debug_implementations,
    missing_docs
)]
#![deny(unreachable_pub, private_in_public)]
#![allow(elided_lifetimes_in_paths, clippy::type_complexity)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(test, allow(clippy::float_cmp))]

use axum::{
    body::{boxed, BoxBody, Bytes, Full, HttpBody},
    extract::{FromRequest, RequestParts},
    http::{Request, Response, StatusCode},
    response::IntoResponse,
    BoxError,
};
use pin_project_lite::pin_project;
use std::{
    convert::Infallible,
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower::ServiceExt;
use tower_layer::Layer;
use tower_service::Service;

/// [`Layer`] that applies [`HandleError`] which is a [`Service`] adapter
/// that handles errors by converting them into responses.
///
/// See [module docs](self) for more details on axum's error handling model.
pub struct HandleErrorLayer<F, T> {
    f: F,
    _extractor: PhantomData<fn() -> T>,
}

impl<F, T> HandleErrorLayer<F, T> {
    /// Create a new `HandleErrorLayer`.
    pub fn new(f: F) -> Self {
        Self {
            f,
            _extractor: PhantomData,
        }
    }
}

impl<F, T> Clone for HandleErrorLayer<F, T>
where
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            f: self.f.clone(),
            _extractor: PhantomData,
        }
    }
}

impl<F, E> fmt::Debug for HandleErrorLayer<F, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandleErrorLayer")
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}

impl<S, F, T> Layer<S> for HandleErrorLayer<F, T>
where
    F: Clone,
{
    type Service = HandleError<S, F, T>;

    fn layer(&self, inner: S) -> Self::Service {
        HandleError::new(inner, self.f.clone())
    }
}

/// A [`Service`] adapter that handles errors by converting them into responses.
///
/// See [module docs](self) for more details on axum's error handling model.
pub struct HandleError<S, F, T> {
    inner: S,
    f: F,
    _extractor: PhantomData<fn() -> T>,
}

impl<S, F, T> HandleError<S, F, T> {
    /// Create a new `HandleError`.
    pub fn new(inner: S, f: F) -> Self {
        Self {
            inner,
            f,
            _extractor: PhantomData,
        }
    }
}

impl<S, F, T> Clone for HandleError<S, F, T>
where
    S: Clone,
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            f: self.f.clone(),
            _extractor: PhantomData,
        }
    }
}

impl<S, F, E> fmt::Debug for HandleError<S, F, E>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandleError")
            .field("inner", &self.inner)
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}

impl<S, F, ReqBody, ResBody, Fut, Res> Service<Request<ReqBody>> for HandleError<S, F, ()>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Error: Send,
    S::Future: Send,
    F: FnOnce(S::Error) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    ReqBody: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = ResponseFuture;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let f = self.f.clone();

        let clone = self.inner.clone();
        let inner = std::mem::replace(&mut self.inner, clone);

        let future = Box::pin(async move {
            match inner.oneshot(req).await {
                Ok(res) => Ok(res.map(boxed)),
                Err(err) => Ok(f(err).await.into_response().map(boxed)),
            }
        });

        ResponseFuture { future }
    }
}

#[allow(unused_macros)]
macro_rules! impl_service {
    ( $($ty:ident),* $(,)? ) => {
        impl<S, F, ReqBody, ResBody, Res, Fut, $($ty,)*> Service<Request<ReqBody>>
            for HandleError<S, F, ($($ty,)*)>
        where
            S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
            S::Error: Send,
            S::Future: Send,
            F: FnOnce($($ty),*, S::Error) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = Res> + Send,
            Res: IntoResponse,
            $( $ty: FromRequest<ReqBody> + Send,)*
            ReqBody: Send + 'static,
            ResBody: HttpBody<Data = Bytes> + Send + 'static,
            ResBody::Error: Into<BoxError>,
        {
            type Response = Response<BoxBody>;
            type Error = Infallible;

            type Future = ResponseFuture;

            fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                Poll::Ready(Ok(()))
            }

            #[allow(non_snake_case)]
            fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
                let f = self.f.clone();

                let clone = self.inner.clone();
                let inner = std::mem::replace(&mut self.inner, clone);

                let future = Box::pin(async move {
                    let mut req = RequestParts::new(req);

                    $(
                        let $ty = match $ty::from_request(&mut req).await {
                            Ok(value) => value,
                            Err(rejection) => return Ok(rejection.into_response().map(boxed)),
                        };
                    )*

                    let req = match req.try_into_request() {
                        Ok(req) => req,
                        Err(err) => {
                            return Ok(Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .body(boxed(Full::from(err.to_string())))
                                .unwrap());
                        }
                    };

                    match inner.oneshot(req).await {
                        Ok(res) => Ok(res.map(boxed)),
                        Err(err) => Ok(f($($ty),*, err).await.into_response().map(boxed)),
                    }
                });

                ResponseFuture { future }
            }
        }
    }
}

impl_service!(T1);
impl_service!(T1, T2);
impl_service!(T1, T2, T3);
impl_service!(T1, T2, T3, T4);
impl_service!(T1, T2, T3, T4, T5);
impl_service!(T1, T2, T3, T4, T5, T6);
impl_service!(T1, T2, T3, T4, T5, T6, T7);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);

pin_project! {
    /// Response future for [`HandleError`].
    pub struct ResponseFuture {
        #[pin]
        future: Pin<Box<dyn Future<Output = Result<Response<BoxBody>, Infallible>> + Send + 'static>>,
    }
}

impl Future for ResponseFuture {
    type Output = Result<Response<BoxBody>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().future.poll(cx)
    }
}

/// Extension trait to [`Service`] for handling errors by mapping them to
/// responses.
///
/// See [module docs](self) for more details on axum's error handling model.
pub trait HandleErrorExt<B>: Service<Request<B>> + Sized {
    /// Apply a [`HandleError`] middleware.
    fn handle_error<F>(self, f: F) -> HandleError<Self, F, B> {
        HandleError::new(self, f)
    }
}

impl<B, S> HandleErrorExt<B> for S where S: Service<Request<B>> {}
