//! Error handling model and utilities
//!
//! # axum's error handling model
//!
//! axum is based on [`tower::Service`] which bundles errors through its associated
//! `Error` type. If you have a [`Service`] that produces an error and that error
//! makes it all the way up to hyper, the connection will be terminated _without_
//! sending a response. This is generally not desirable so axum makes sure you
//! always produce a response by relying on the type system.
//!
//! axum does this by requiring all services have [`Infallible`] as their error
//! type. `Infallible` is the error type for errors that can never happen.
//!
//! This means if you define a handler like:
//!
//! ```rust
//! use axum::http::StatusCode;
//!
//! async fn handler() -> Result<String, StatusCode> {
//!     # todo!()
//!     // ...
//! }
//! ```
//!
//! While it looks like it might fail with a `StatusCode` this actually isn't an
//! "error". If this handler returns `Err(some_status_code)` that will still be
//! converted into a [`Response`] and sent back to the client. This is done
//! through `StatusCode`'s [`IntoResponse`] implementation.
//!
//! It doesn't matter whether you return `Err(StatusCode::NOT_FOUND)` or
//! `Err(StatusCode::INTERNAL_SERVER_ERROR)`. These are not considered errors in
//! axum.
//!
//! Instead of a direct `StatusCode`, it makes sense to use intermediate error type
//! that can ultimately be converted to `Response`. This allows using `?` operator
//! in handlers. See those examples:
//!
//! * [`anyhow-error-response`][anyhow] for generic boxed errors
//! * [`error-handling`][error-handling] for application-specific detailed errors
//!
//! [anyhow]: https://github.com/tokio-rs/axum/blob/main/examples/anyhow-error-response/src/main.rs
//! [error-handling]: https://github.com/tokio-rs/axum/blob/main/examples/error-handling/src/main.rs
//!
//! This also applies to extractors. If an extractor doesn't match the request the
//! request will be rejected and a response will be returned without calling your
//! handler. See [`extract`](crate::extract) to learn more about handling extractor
//! failures.
//!
//! # Routing to fallible services
//!
//! You generally don't have to think about errors if you're only using async
//! functions as handlers. However if you're embedding general `Service`s or
//! applying middleware, which might produce errors you have to tell axum how to
//! convert those errors into responses.
//!
//! ```rust
//! use axum::{
//!     Router,
//!     body::Body,
//!     http::{Request, Response, StatusCode},
//!     error_handling::HandleError,
//! };
//!
//! async fn thing_that_might_fail() -> Result<(), anyhow::Error> {
//!     # Ok(())
//!     // ...
//! }
//!
//! // this service might fail with `anyhow::Error`
//! let some_fallible_service = tower::service_fn(|_req| async {
//!     thing_that_might_fail().await?;
//!     Ok::<_, anyhow::Error>(Response::new(Body::empty()))
//! });
//!
//! let app = Router::new().route_service(
//!     "/",
//!     // we cannot route to `some_fallible_service` directly since it might fail.
//!     // we have to use `handle_error` which converts its errors into responses
//!     // and changes its error type from `anyhow::Error` to `Infallible`.
//!     HandleError::new(some_fallible_service, handle_anyhow_error),
//! );
//!
//! // handle errors by converting them into something that implements
//! // `IntoResponse`
//! async fn handle_anyhow_error(err: anyhow::Error) -> (StatusCode, String) {
//!     (
//!         StatusCode::INTERNAL_SERVER_ERROR,
//!         format!("Something went wrong: {err}"),
//!     )
//! }
//! # let _: Router = app;
//! ```
//!
//! # Applying fallible middleware
//!
//! Similarly axum requires you to handle errors from middleware. That is done with
//! [`HandleErrorLayer`]:
//!
//! ```rust
//! use axum::{
//!     Router,
//!     BoxError,
//!     routing::get,
//!     http::StatusCode,
//!     error_handling::HandleErrorLayer,
//! };
//! use std::time::Duration;
//! use tower::ServiceBuilder;
//!
//! let app = Router::new()
//!     .route("/", get(|| async {}))
//!     .layer(
//!         ServiceBuilder::new()
//!             // `timeout` will produce an error if the handler takes
//!             // too long so we must handle those
//!             .layer(HandleErrorLayer::new(handle_timeout_error))
//!             .timeout(Duration::from_secs(30))
//!     );
//!
//! async fn handle_timeout_error(err: BoxError) -> (StatusCode, String) {
//!     if err.is::<tower::timeout::error::Elapsed>() {
//!         (
//!             StatusCode::REQUEST_TIMEOUT,
//!             "Request took too long".to_string(),
//!         )
//!     } else {
//!         (
//!             StatusCode::INTERNAL_SERVER_ERROR,
//!             format!("Unhandled internal error: {err}"),
//!         )
//!     }
//! }
//! # let _: Router = app;
//! ```
//!
//! # Running extractors for error handling
//!
//! `HandleErrorLayer` also supports running extractors:
//!
//! ```rust
//! use axum::{
//!     Router,
//!     BoxError,
//!     routing::get,
//!     http::{StatusCode, Method, Uri},
//!     error_handling::HandleErrorLayer,
//! };
//! use std::time::Duration;
//! use tower::ServiceBuilder;
//!
//! let app = Router::new()
//!     .route("/", get(|| async {}))
//!     .layer(
//!         ServiceBuilder::new()
//!             // `timeout` will produce an error if the handler takes
//!             // too long so we must handle those
//!             .layer(HandleErrorLayer::new(handle_timeout_error))
//!             .timeout(Duration::from_secs(30))
//!     );
//!
//! async fn handle_timeout_error(
//!     // `Method` and `Uri` are extractors so they can be used here
//!     method: Method,
//!     uri: Uri,
//!     // the last argument must be the error itself
//!     err: BoxError,
//! ) -> (StatusCode, String) {
//!     (
//!         StatusCode::INTERNAL_SERVER_ERROR,
//!         format!("`{method} {uri}` failed with {err}"),
//!     )
//! }
//! # let _: Router = app;
//! ```
//!
//! [`tower::Service`]: `tower::Service`
//! [`Infallible`]: std::convert::Infallible
//! [`Response`]: crate::response::Response
//! [`IntoResponse`]: crate::response::IntoResponse

use crate::{
    extract::FromRequestParts,
    http::Request,
    response::{IntoResponse, Response},
};
use std::{
    convert::Infallible,
    fmt,
    future::Future,
    marker::PhantomData,
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

impl<S, F, B, Fut, Res> Service<Request<B>> for HandleError<S, F, ()>
where
    S: Service<Request<B>> + Clone + Send + 'static,
    S::Response: IntoResponse + Send,
    S::Error: Send,
    S::Future: Send,
    F: FnOnce(S::Error) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = future::HandleErrorFuture;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let f = self.f.clone();

        let clone = self.inner.clone();
        let inner = std::mem::replace(&mut self.inner, clone);

        let future = Box::pin(async move {
            match inner.oneshot(req).await {
                Ok(res) => Ok(res.into_response()),
                Err(err) => Ok(f(err).await.into_response()),
            }
        });

        future::HandleErrorFuture { future }
    }
}

#[allow(unused_macros)]
macro_rules! impl_service {
    ( $($ty:ident),* $(,)? ) => {
        impl<S, F, B, Res, Fut, $($ty,)*> Service<Request<B>>
            for HandleError<S, F, ($($ty,)*)>
        where
            S: Service<Request<B>> + Clone + Send + 'static,
            S::Response: IntoResponse + Send,
            S::Error: Send,
            S::Future: Send,
            F: FnOnce($($ty),*, S::Error) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = Res> + Send,
            Res: IntoResponse,
            $( $ty: FromRequestParts<()> + Send,)*
            B: Send + 'static,
        {
            type Response = Response;
            type Error = Infallible;

            type Future = future::HandleErrorFuture;

            fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                Poll::Ready(Ok(()))
            }

            #[allow(non_snake_case)]
            fn call(&mut self, req: Request<B>) -> Self::Future {
                let f = self.f.clone();

                let clone = self.inner.clone();
                let inner = std::mem::replace(&mut self.inner, clone);

                let (mut parts, body) = req.into_parts();

                let future = Box::pin(async move {
                    $(
                        let $ty = match $ty::from_request_parts(&mut parts, &()).await {
                            Ok(value) => value,
                            Err(rejection) => return Ok(rejection.into_response()),
                        };
                    )*

                    let req = Request::from_parts(parts, body);

                    match inner.oneshot(req).await {
                        Ok(res) => Ok(res.into_response()),
                        Err(err) => Ok(f($($ty),*, err).await.into_response()),
                    }
                });

                future::HandleErrorFuture { future }
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

pub mod future {
    //! Future types.

    use crate::response::Response;
    use pin_project_lite::pin_project;
    use std::{
        convert::Infallible,
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    };

    pin_project! {
        /// Response future for [`HandleError`].
        pub struct HandleErrorFuture {
            #[pin]
            pub(super) future: Pin<Box<dyn Future<Output = Result<Response, Infallible>>
                + Send
                + 'static
            >>,
        }
    }

    impl Future for HandleErrorFuture {
        type Output = Result<Response, Infallible>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            self.project().future.poll(cx)
        }
    }
}

#[test]
fn traits() {
    use crate::test_helpers::*;

    assert_send::<HandleError<(), (), NotSendSync>>();
    assert_sync::<HandleError<(), (), NotSendSync>>();
}
