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
    body::Body,
    extract::{connect_info::IntoMakeServiceWithConnectInfo, FromRequest, FromRequestParts},
    response::{IntoResponse, Response},
    routing::IntoMakeService,
};
use http::Request;
use std::{convert::Infallible, fmt, future::Future, marker::PhantomData, pin::Pin, sync::Arc};
use tower::ServiceExt;
use tower_layer::Layer;
use tower_service::Service;

pub mod future;
mod into_service;
mod into_service_state_in_extension;
mod with_state;

pub(crate) use self::into_service_state_in_extension::IntoServiceStateInExtension;
pub use self::{into_service::IntoService, with_state::WithState};

/// Trait for async functions that can be used to handle requests.
///
/// You shouldn't need to depend on this trait directly. It is automatically
/// implemented to closures of the right types.
///
/// See the [module docs](crate::handler) for more details.
///
/// # Converting `Handler`s into [`Service`]s
///
/// To convert `Handler`s into [`Service`]s you have to call either
/// [`HandlerWithoutStateExt::into_service`] or [`Handler::with_state`]:
///
/// ```
/// use tower::Service;
/// use axum::{
///     extract::State,
///     body::Body,
///     http::Request,
///     handler::{HandlerWithoutStateExt, Handler},
/// };
///
/// // this handler doesn't require any state
/// async fn one() {}
/// // so it can be converted to a service with `HandlerWithoutStateExt::into_service`
/// assert_service(one.into_service());
///
/// // this handler requires state
/// async fn two(_: State<String>) {}
/// // so we have to provide it
/// let handler_with_state = two.with_state(String::new());
/// // which gives us a `Service`
/// assert_service(handler_with_state);
///
/// // helper to check that a value implements `Service`
/// fn assert_service<S>(service: S)
/// where
///     S: Service<Request<Body>>,
/// {}
/// ```
#[doc = include_str!("../docs/debugging_handler_type_errors.md")]
pub trait Handler<T, S, B = Body>: Clone + Send + Sized + 'static {
    /// The type of future calling this handler returns.
    type Future: Future<Output = Response> + Send + 'static;

    /// Call the handler with the given request.
    fn call(self, req: Request<B>, state: Arc<S>) -> Self::Future;

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
    fn layer<L>(self, layer: L) -> Layered<L, Self, T, S, B>
    where
        L: Layer<WithState<Self, T, S, B>>,
    {
        Layered {
            layer,
            handler: self,
            _marker: PhantomData,
        }
    }

    /// Convert the handler into a [`Service`] by providing the state
    fn with_state(self, state: S) -> WithState<Self, T, S, B> {
        self.with_state_arc(Arc::new(state))
    }

    /// Convert the handler into a [`Service`] by providing the state
    fn with_state_arc(self, state: Arc<S>) -> WithState<Self, T, S, B> {
        WithState {
            service: IntoService::new(self, state),
        }
    }
}

impl<F, Fut, Res, S, B> Handler<((),), S, B> for F
where
    F: FnOnce() -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

    fn call(self, _req: Request<B>, _state: Arc<S>) -> Self::Future {
        Box::pin(async move { self().await.into_response() })
    }
}

macro_rules! impl_handler {
    (
        [$($ty:ident),*], $last:ident
    ) => {
        #[allow(non_snake_case, unused_mut)]
        impl<F, Fut, S, B, Res, M, $($ty,)* $last> Handler<(M, $($ty,)* $last,), S, B> for F
        where
            F: FnOnce($($ty,)* $last,) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = Res> + Send,
            B: Send + 'static,
            S: Send + Sync + 'static,
            Res: IntoResponse,
            $( $ty: FromRequestParts<S> + Send, )*
            $last: FromRequest<S, B, M> + Send,
        {
            type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

            fn call(self, req: Request<B>, state: Arc<S>) -> Self::Future {
                Box::pin(async move {
                    let (mut parts, body) = req.into_parts();
                    let state = &state;

                    $(
                        let $ty = match $ty::from_request_parts(&mut parts, state).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_response(),
                        };
                    )*

                    let req = Request::from_parts(parts, body);

                    let $last = match $last::from_request(req, state).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_response(),
                    };

                    let res = self($($ty,)* $last,).await;

                    res.into_response()
                })
            }
        }
    };
}

impl_handler!([], T1);
impl_handler!([T1], T2);
impl_handler!([T1, T2], T3);
impl_handler!([T1, T2, T3], T4);
impl_handler!([T1, T2, T3, T4], T5);
impl_handler!([T1, T2, T3, T4, T5], T6);
impl_handler!([T1, T2, T3, T4, T5, T6], T7);
impl_handler!([T1, T2, T3, T4, T5, T6, T7], T8);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8], T9);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9], T10);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10], T11);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11], T12);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12], T13);
impl_handler!(
    [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13],
    T14
);
impl_handler!(
    [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14],
    T15
);
impl_handler!(
    [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15],
    T16
);

/// A [`Service`] created from a [`Handler`] by applying a Tower middleware.
///
/// Created with [`Handler::layer`]. See that method for more details.
pub struct Layered<L, H, T, S, B> {
    layer: L,
    handler: H,
    _marker: PhantomData<fn() -> (T, S, B)>,
}

impl<L, H, T, S, B> fmt::Debug for Layered<L, H, T, S, B>
where
    L: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Layered")
            .field("layer", &self.layer)
            .finish()
    }
}

impl<L, H, T, S, B> Clone for Layered<L, H, T, S, B>
where
    L: Clone,
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            layer: self.layer.clone(),
            handler: self.handler.clone(),
            _marker: PhantomData,
        }
    }
}

impl<H, S, T, B, L> Handler<T, S, B> for Layered<L, H, T, S, B>
where
    L: Layer<WithState<H, T, S, B>> + Clone + Send + 'static,
    H: Handler<T, S, B>,
    L::Service: Service<Request<B>, Error = Infallible> + Clone + Send + 'static,
    <L::Service as Service<Request<B>>>::Response: IntoResponse,
    <L::Service as Service<Request<B>>>::Future: Send,
    T: 'static,
    S: 'static,
    B: Send + 'static,
{
    type Future = future::LayeredFuture<B, L::Service>;

    fn call(self, req: Request<B>, state: Arc<S>) -> Self::Future {
        use futures_util::future::{FutureExt, Map};

        let svc = self.handler.with_state_arc(state);
        let svc = self.layer.layer(svc);

        let future: Map<
            _,
            fn(
                Result<
                    <L::Service as Service<Request<B>>>::Response,
                    <L::Service as Service<Request<B>>>::Error,
                >,
            ) -> _,
        > = svc.oneshot(req).map(|result| match result {
            Ok(res) => res.into_response(),
            Err(err) => match err {},
        });

        future::LayeredFuture::new(future)
    }
}

/// Extension trait for [`Handler`]s that don't have state.
///
/// This provides convenience methods to convert the [`Handler`] into a [`Service`] or [`MakeService`].
///
/// [`MakeService`]: tower::make::MakeService
pub trait HandlerWithoutStateExt<T, B>: Handler<T, (), B> {
    /// Convert the handler into a [`Service`] and no state.
    fn into_service(self) -> WithState<Self, T, (), B>;

    /// Convert the handler into a [`MakeService`] and no state.
    ///
    /// See [`WithState::into_make_service`] for more details.
    ///
    /// [`MakeService`]: tower::make::MakeService
    fn into_make_service(self) -> IntoMakeService<IntoService<Self, T, (), B>>;

    /// Convert the handler into a [`MakeService`] which stores information
    /// about the incoming connection and has no state.
    ///
    /// See [`WithState::into_make_service_with_connect_info`] for more details.
    ///
    /// [`MakeService`]: tower::make::MakeService
    fn into_make_service_with_connect_info<C>(
        self,
    ) -> IntoMakeServiceWithConnectInfo<IntoService<Self, T, (), B>, C>;
}

impl<H, T, B> HandlerWithoutStateExt<T, B> for H
where
    H: Handler<T, (), B>,
{
    fn into_service(self) -> WithState<Self, T, (), B> {
        self.with_state(())
    }

    fn into_make_service(self) -> IntoMakeService<IntoService<Self, T, (), B>> {
        self.with_state(()).into_make_service()
    }

    fn into_make_service_with_connect_info<C>(
        self,
    ) -> IntoMakeServiceWithConnectInfo<IntoService<Self, T, (), B>, C> {
        self.with_state(()).into_make_service_with_connect_info()
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
}
