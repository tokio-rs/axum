//! Async functions that can be used to handle requests.
//!
#![doc = include_str!("../docs/handlers_intro.md")]
//!
//! Some examples of handlers:
//!
//! ```rust
//! use axum::body::Bytes;
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
#![doc = include_str!("../docs/debugging_handler_type_errors.md")]

use crate::{
    body::{boxed, Body, Bytes, HttpBody},
    extract::{connect_info::IntoMakeServiceWithConnectInfo, FromRequest, RequestParts},
    response::{IntoResponse, Response},
    routing::IntoMakeService,
    BoxError,
};
use http::Request;
use std::{fmt, future::Future, marker::PhantomData, pin::Pin};
use tower::ServiceExt;
use tower_layer::Layer;
use tower_service::Service;

pub mod future;
mod into_service;
mod into_service_state_in_extension;

pub use self::into_service::IntoService;
pub(crate) use self::into_service_state_in_extension::IntoServiceStateInExtension;

/// Trait for async functions that can be used to handle requests.
///
/// You shouldn't need to depend on this trait directly. It is automatically
/// implemented to closures of the right types.
///
/// See the [module docs](crate::handler) for more details.
///
#[doc = include_str!("../docs/debugging_handler_type_errors.md")]
pub trait Handler<T, S, B = Body>: Clone + Send + Sized + 'static {
    /// The type of future calling this handler returns.
    type Future: Future<Output = Response> + Send + 'static;

    /// Call the handler with the given request.
    fn call(self, state: S, req: Request<B>) -> Self::Future;

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
    fn layer<L>(self, layer: L) -> Layered<L::Service, T, S>
    where
        L: Layer<IntoService<Self, T, S, B>>,
    {
        todo!()
        // Layered::new(layer.layer(self.into_service()))
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
    /// async fn handler(method: Method, uri: Uri) -> (StatusCode, String) {
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
    fn into_service(self, state: S) -> IntoService<Self, T, S, B> {
        IntoService::new(self, state)
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
    /// async fn handler(method: Method, uri: Uri, body: String) -> String {
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
    fn into_make_service(self, state: S) -> IntoMakeService<IntoService<Self, T, S, B>> {
        IntoMakeService::new(self.into_service(state))
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
    /// async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
    ///     format!("Hello {}", addr)
    /// }
    ///
    /// # async {
    /// Server::bind(&SocketAddr::from(([127, 0, 0, 1], 3000)))
    ///     .serve(handler.into_make_service_with_connect_info::<SocketAddr>())
    ///     .await?;
    /// # Ok::<_, hyper::Error>(())
    /// # };
    /// ```
    ///
    /// [`MakeService`]: tower::make::MakeService
    /// [`Router::into_make_service_with_connect_info`]: crate::routing::Router::into_make_service_with_connect_info
    fn into_make_service_with_connect_info<C>(
        self,
        state: S,
    ) -> IntoMakeServiceWithConnectInfo<IntoService<Self, T, S, B>, C> {
        IntoMakeServiceWithConnectInfo::new(self.into_service(state))
    }
}

impl<F, Fut, Res, S, B> Handler<(), S, B> for F
where
    F: FnOnce() -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

    fn call(self, _state: S, _req: Request<B>) -> Self::Future {
        Box::pin(async move { self().await.into_response() })
    }
}

macro_rules! impl_handler {
    ( $($ty:ident),* $(,)? ) => {
        #[allow(non_snake_case)]
        impl<F, Fut, B, S, Res, $($ty,)*> Handler<($($ty,)*), S, B> for F
        where
            F: FnOnce($($ty,)*) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = Res> + Send,
            B: Send + 'static,
            S: Send + 'static,
            Res: IntoResponse,
            $( $ty: FromRequest<S, B> + Send,)*
        {
            type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

            fn call(self, state: S, req: Request<B>) -> Self::Future {
                Box::pin(async move {
                    let mut req = RequestParts::new(state, req);

                    $(
                        let $ty = match $ty::from_request(&mut req).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_response(),
                        };
                    )*

                    let res = self($($ty,)*).await;

                    res.into_response()
                })
            }
        }
    };
}

all_the_tuples!(impl_handler);

/// A [`Service`] created from a [`Handler`] by applying a Tower middleware.
///
/// Created with [`Handler::layer`]. See that method for more details.
pub struct Layered<Svc, T, S> {
    svc: Svc,
    _input: PhantomData<fn() -> (T, S)>,
}

impl<Svc, T, S> fmt::Debug for Layered<Svc, T, S>
where
    Svc: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Layered").field("svc", &self.svc).finish()
    }
}

impl<Svc, T, S> Clone for Layered<Svc, T, S>
where
    Svc: Clone,
{
    fn clone(&self) -> Self {
        Self::new(self.svc.clone())
    }
}

impl<Svc, S, T, ReqBody, ResBody> Handler<T, S, ReqBody> for Layered<Svc, T, S>
where
    Svc: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    Svc::Error: IntoResponse,
    Svc::Future: Send,
    T: 'static,
    S: 'static,
    ReqBody: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
{
    type Future = future::LayeredFuture<Svc, ReqBody>;

    fn call(self, state: S, req: Request<ReqBody>) -> Self::Future {
        todo!()

        // use futures_util::future::{FutureExt, Map};

        // let future: Map<_, fn(Result<Svc::Response, Svc::Error>) -> _> =
        //     self.svc.oneshot(req).map(|result| match result {
        //         Ok(res) => res.map(boxed),
        //         Err(res) => res.into_response(),
        //     });

        // future::LayeredFuture::new(future)
    }
}

impl<Svc, T, S> Layered<Svc, T, S> {
    pub(crate) fn new(svc: Svc) -> Self {
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

        let client = TestClient::new(handle.into_service(()));

        let res = client.post("/").body("hi there!").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "you said: hi there!");
    }
}
