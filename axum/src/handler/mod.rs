//! Async functions that can be used to handle requests.
//!
#![doc = include_str!("../docs/handlers_intro.md")]
//!
//! Some examples of handlers:
//!
//! ```rust
//! use bytes::Bytes;
//! use http::StatusCode;
//!
//! // Handler that immediately returns an empty `200 OK` response.
//! async fn unit_handler() {}
//!
//! // Handler that immediately returns an empty `200 OK` response with a plain
//! // text body.
//! async fn string_handler() -> String {
//!     "Hello, World!".to_string()
//! }
//!
//! // Handler that buffers the request body and returns it.
//! //
//! // This works because `Bytes` implements `FromRequest`
//! // and therefore can be used as an extractor.
//! //
//! // `String` and `StatusCode` both implement `IntoResponse` and
//! // therefore `Result<String, StatusCode>` also implements `IntoResponse`
//! async fn echo(body: Bytes) -> Result<String, StatusCode> {
//!     if let Ok(string) = String::from_utf8(body.to_vec()) {
//!         Ok(string)
//!     } else {
//!         Err(StatusCode::BAD_REQUEST)
//!     }
//! }
//! ```
//!
//! ## Debugging handler type errors
//!
//! For a function to be used as a handler it must implement the [`Handler`] trait.
//! axum provides blanket implementations for functions that:
//!
//! - Are `async fn`s.
//! - Take no more than 16 arguments that all implement [`FromRequest`].
//! - Returns something that implements [`IntoResponse`].
//! - If a closure is used it must implement `Clone + Send + Sync` and be
//! `'static`.
//! - Returns a future that is `Send`. The most common way to accidentally make a
//! future `!Send` is to hold a `!Send` type across an await.
//!
//! Unfortunately Rust gives poor error messages if you try to use a function
//! that doesn't quite match what's required by [`Handler`].
//!
//! You might get an error like this:
//!
//! ```not_rust
//! error[E0277]: the trait bound `fn(bool) -> impl Future {handler}: Handler<_, _>` is not satisfied
//!    --> src/main.rs:13:44
//!     |
//! 13  |     let app = Router::new().route("/", get(handler));
//!     |                                            ^^^^^^^ the trait `Handler<_, _>` is not implemented for `fn(bool) -> impl Future {handler}`
//!     |
//!    ::: axum/src/handler/mod.rs:116:8
//!     |
//! 116 |     H: Handler<B, T>,
//!     |        ------------- required by this bound in `axum::routing::get`
//! ```
//!
//! This error doesn't tell you _why_ your function doesn't implement
//! [`Handler`]. It's possible to improve the error with the [`debug_handler`]
//! proc-macro from the [axum-debug] crate.
//!
//! [axum-debug]: https://docs.rs/axum-debug

use crate::{
    body::{boxed, BoxBody},
    extract::{
        connect_info::{Connected, IntoMakeServiceWithConnectInfo},
        FromRequest, RequestParts,
    },
    response::IntoResponse,
    routing::IntoMakeService,
    BoxError,
};
use async_trait::async_trait;
use bytes::Bytes;
use http::{Request, Response};
use std::{fmt, future::Future, marker::PhantomData};
use tower::ServiceExt;
use tower_layer::Layer;
use tower_service::Service;

pub mod future;
mod into_service;

pub use self::into_service::IntoService;

pub(crate) mod sealed {
    #![allow(unreachable_pub, missing_docs, missing_debug_implementations)]

    pub trait HiddenTrait {}
    pub struct Hidden;
    impl HiddenTrait for Hidden {}
}

/// Trait for async functions that can be used to handle requests.
///
/// You shouldn't need to depend on this trait directly. It is automatically
/// implemented to closures of the right types.
///
/// See the [module docs](crate::handler) for more details.
#[async_trait]
pub trait Handler<B, T>: Clone + Send + Sized + 'static {
    // This seals the trait. We cannot use the regular "sealed super trait"
    // approach due to coherence.
    #[doc(hidden)]
    type Sealed: sealed::HiddenTrait;

    /// Call the handler with the given request.
    async fn call(self, req: Request<B>) -> Response<BoxBody>;

    /// Apply a [`tower::Layer`] to the handler.
    ///
    /// All requests to the handler will be processed by the layer's
    /// corresponding middleware.
    ///
    /// This can be used to add additional processing to a request for a single
    /// handler.
    ///
    /// Note this differs from [`routing::Router::layer`](crate::routing::Router::layer)
    /// which adds a middleware to a group of routes.
    ///
    /// If you're applying middleware that produces errors you have to handle the errors
    /// so they're converted into responses. You can learn more about doing that
    /// [here](crate::error_handling).
    ///
    /// # Example
    ///
    /// Adding the [`tower::limit::ConcurrencyLimit`] middleware to a handler
    /// can be done like so:
    ///
    /// ```rust
    /// use axum::{
    ///     routing::get,
    ///     handler::Handler,
    ///     Router,
    /// };
    /// use tower::limit::{ConcurrencyLimitLayer, ConcurrencyLimit};
    ///
    /// async fn handler() { /* ... */ }
    ///
    /// let layered_handler = handler.layer(ConcurrencyLimitLayer::new(64));
    /// let app = Router::new().route("/", get(layered_handler));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    fn layer<L>(self, layer: L) -> Layered<L::Service, T>
    where
        L: Layer<IntoService<Self, B, T>>,
    {
        Layered::new(layer.layer(self.into_service()))
    }

    /// Convert the handler into a [`Service`].
    ///
    /// This is commonly used together with [`Router::fallback`]:
    ///
    /// ```rust
    /// use axum::{
    ///     Server,
    ///     handler::Handler,
    ///     http::{Uri, Method, StatusCode},
    ///     response::IntoResponse,
    ///     routing::{get, Router},
    /// };
    /// use tower::make::Shared;
    /// use std::net::SocketAddr;
    ///
    /// async fn handler(method: Method, uri: Uri) -> impl IntoResponse {
    ///     (StatusCode::NOT_FOUND, format!("Nothing to see at {} {}", method, uri))
    /// }
    ///
    /// let app = Router::new()
    ///     .route("/", get(|| async {}))
    ///     .fallback(handler.into_service());
    ///
    /// # async {
    /// Server::bind(&SocketAddr::from(([127, 0, 0, 1], 3000)))
    ///     .serve(app.into_make_service())
    ///     .await?;
    /// # Ok::<_, hyper::Error>(())
    /// # };
    /// ```
    ///
    /// [`Router::fallback`]: crate::routing::Router::fallback
    fn into_service(self) -> IntoService<Self, B, T> {
        IntoService::new(self)
    }

    /// Convert the handler into a [`MakeService`].
    ///
    /// This allows you to serve a single handler if you don't need any routing:
    ///
    /// ```rust
    /// use axum::{
    ///     Server, handler::Handler, http::{Uri, Method}, response::IntoResponse,
    /// };
    /// use std::net::SocketAddr;
    ///
    /// async fn handler(method: Method, uri: Uri, body: String) -> impl IntoResponse {
    ///     format!("received `{} {}` with body `{:?}`", method, uri, body)
    /// }
    ///
    /// # async {
    /// Server::bind(&SocketAddr::from(([127, 0, 0, 1], 3000)))
    ///     .serve(handler.into_make_service())
    ///     .await?;
    /// # Ok::<_, hyper::Error>(())
    /// # };
    /// ```
    ///
    /// [`MakeService`]: tower::make::MakeService
    fn into_make_service(self) -> IntoMakeService<IntoService<Self, B, T>> {
        IntoMakeService::new(self.into_service())
    }

    /// Convert the handler into a [`MakeService`] which stores information
    /// about the incoming connection.
    ///
    /// See [`Router::into_make_service_with_connect_info`] for more details.
    ///
    /// ```rust
    /// use axum::{
    ///     Server,
    ///     handler::Handler,
    ///     response::IntoResponse,
    ///     extract::ConnectInfo,
    /// };
    /// use std::net::SocketAddr;
    ///
    /// async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> impl IntoResponse {
    ///     format!("Hello {}", addr)
    /// }
    ///
    /// # async {
    /// Server::bind(&SocketAddr::from(([127, 0, 0, 1], 3000)))
    ///     .serve(handler.into_make_service_with_connect_info::<SocketAddr, _>())
    ///     .await?;
    /// # Ok::<_, hyper::Error>(())
    /// # };
    /// ```
    ///
    /// [`MakeService`]: tower::make::MakeService
    /// [`Router::into_make_service_with_connect_info`]: crate::routing::Router::into_make_service_with_connect_info
    fn into_make_service_with_connect_info<C, Target>(
        self,
    ) -> IntoMakeServiceWithConnectInfo<IntoService<Self, B, T>, C>
    where
        C: Connected<Target>,
    {
        IntoMakeServiceWithConnectInfo::new(self.into_service())
    }
}

#[async_trait]
impl<F, Fut, Res, B> Handler<B, ()> for F
where
    F: FnOnce() -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
{
    type Sealed = sealed::Hidden;

    async fn call(self, _req: Request<B>) -> Response<BoxBody> {
        self().await.into_response().map(boxed)
    }
}

macro_rules! impl_handler {
    ( $($ty:ident),* $(,)? ) => {
        #[async_trait]
        #[allow(non_snake_case)]
        impl<F, Fut, B, Res, $($ty,)*> Handler<B, ($($ty,)*)> for F
        where
            F: FnOnce($($ty,)*) -> Fut + Clone + Send + Sync + 'static,
            Fut: Future<Output = Res> + Send,
            B: Send + 'static,
            Res: IntoResponse,
            $( $ty: FromRequest<B> + Send,)*
        {
            type Sealed = sealed::Hidden;

            async fn call(self, req: Request<B>) -> Response<BoxBody> {
                let mut req = RequestParts::new(req);

                $(
                    let $ty = match $ty::from_request(&mut req).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_response().map(boxed),
                    };
                )*

                let res = self($($ty,)*).await;

                res.into_response().map(boxed)
            }
        }
    };
}

impl_handler!(T1);
impl_handler!(T1, T2);
impl_handler!(T1, T2, T3);
impl_handler!(T1, T2, T3, T4);
impl_handler!(T1, T2, T3, T4, T5);
impl_handler!(T1, T2, T3, T4, T5, T6);
impl_handler!(T1, T2, T3, T4, T5, T6, T7);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);

/// A [`Service`] created from a [`Handler`] by applying a Tower middleware.
///
/// Created with [`Handler::layer`]. See that method for more details.
pub struct Layered<S, T> {
    svc: S,
    _input: PhantomData<fn() -> T>,
}

impl<S, T> fmt::Debug for Layered<S, T>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Layered").field("svc", &self.svc).finish()
    }
}

impl<S, T> Clone for Layered<S, T>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self::new(self.svc.clone())
    }
}

#[async_trait]
impl<S, T, ReqBody, ResBody> Handler<ReqBody, T> for Layered<S, T>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Error: IntoResponse,
    S::Future: Send,
    T: 'static,
    ReqBody: Send + 'static,
    ResBody: http_body::Body<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
{
    type Sealed = sealed::Hidden;

    async fn call(self, req: Request<ReqBody>) -> Response<BoxBody> {
        match self
            .svc
            .oneshot(req)
            .await
            .map_err(IntoResponse::into_response)
        {
            Ok(res) => res.map(boxed),
            Err(res) => res.map(boxed),
        }
    }
}

impl<S, T> Layered<S, T> {
    pub(crate) fn new(svc: S) -> Self {
        Self {
            svc,
            _input: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use http::StatusCode;

    #[tokio::test]
    async fn handler_into_service() {
        async fn handle(body: String) -> impl IntoResponse {
            format!("you said: {}", body)
        }

        let client = TestClient::new(handle.into_service());

        let res = client.post("/").body("hi there!").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "you said: hi there!");
    }

    #[test]
    fn traits() {
        use crate::{routing::MethodRouter, test_helpers::*};
        assert_send::<MethodRouter<(), NotSendSync, NotSendSync, ()>>();
        assert_sync::<MethodRouter<(), NotSendSync, NotSendSync, ()>>();
    }
}
