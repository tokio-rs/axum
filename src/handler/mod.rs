//! Async functions that can be used to handle requests.

use crate::{
    body::{box_body, BoxBody},
    extract::FromRequest,
    response::IntoResponse,
    routing::{future::RouteFuture, EmptyRouter, MethodFilter},
    service::{HandleError, HandleErrorFromRouter},
};
use async_trait::async_trait;
use bytes::Bytes;
use http::{Request, Response};
use std::{
    convert::Infallible,
    fmt,
    future::Future,
    marker::PhantomData,
    task::{Context, Poll},
};
use tower::{BoxError, Layer, Service, ServiceExt};

pub mod future;

/// Route requests to the given handler regardless of the HTTP method of the
/// request.
///
/// # Example
///
/// ```rust
/// use axum::prelude::*;
///
/// async fn handler() {}
///
/// // All requests to `/` will go to `handler` regardless of the HTTP method.
/// let app = route("/", any(handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub fn any<H, B, T>(handler: H) -> OnMethod<IntoService<H, B, T>, EmptyRouter>
where
    H: Handler<B, T>,
{
    on(MethodFilter::all(), handler)
}

/// Route `CONNECT` requests to the given handler.
///
/// See [`get`] for an example.
pub fn connect<H, B, T>(handler: H) -> OnMethod<IntoService<H, B, T>, EmptyRouter>
where
    H: Handler<B, T>,
{
    on(MethodFilter::CONNECT, handler)
}

/// Route `DELETE` requests to the given handler.
///
/// See [`get`] for an example.
pub fn delete<H, B, T>(handler: H) -> OnMethod<IntoService<H, B, T>, EmptyRouter>
where
    H: Handler<B, T>,
{
    on(MethodFilter::DELETE, handler)
}

/// Route `GET` requests to the given handler.
///
/// # Example
///
/// ```rust
/// use axum::prelude::*;
///
/// async fn handler() {}
///
/// // Requests to `GET /` will go to `handler`.
/// let app = route("/", get(handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub fn get<H, B, T>(handler: H) -> OnMethod<IntoService<H, B, T>, EmptyRouter>
where
    H: Handler<B, T>,
{
    on(MethodFilter::GET, handler)
}

/// Route `HEAD` requests to the given handler.
///
/// See [`get`] for an example.
pub fn head<H, B, T>(handler: H) -> OnMethod<IntoService<H, B, T>, EmptyRouter>
where
    H: Handler<B, T>,
{
    on(MethodFilter::HEAD, handler)
}

/// Route `OPTIONS` requests to the given handler.
///
/// See [`get`] for an example.
pub fn options<H, B, T>(handler: H) -> OnMethod<IntoService<H, B, T>, EmptyRouter>
where
    H: Handler<B, T>,
{
    on(MethodFilter::OPTIONS, handler)
}

/// Route `PATCH` requests to the given handler.
///
/// See [`get`] for an example.
pub fn patch<H, B, T>(handler: H) -> OnMethod<IntoService<H, B, T>, EmptyRouter>
where
    H: Handler<B, T>,
{
    on(MethodFilter::PATCH, handler)
}

/// Route `POST` requests to the given handler.
///
/// See [`get`] for an example.
pub fn post<H, B, T>(handler: H) -> OnMethod<IntoService<H, B, T>, EmptyRouter>
where
    H: Handler<B, T>,
{
    on(MethodFilter::POST, handler)
}

/// Route `PUT` requests to the given handler.
///
/// See [`get`] for an example.
pub fn put<H, B, T>(handler: H) -> OnMethod<IntoService<H, B, T>, EmptyRouter>
where
    H: Handler<B, T>,
{
    on(MethodFilter::PUT, handler)
}

/// Route `TRACE` requests to the given handler.
///
/// See [`get`] for an example.
pub fn trace<H, B, T>(handler: H) -> OnMethod<IntoService<H, B, T>, EmptyRouter>
where
    H: Handler<B, T>,
{
    on(MethodFilter::TRACE, handler)
}

/// Route requests with the given method to the handler.
///
/// # Example
///
/// ```rust
/// use axum::{handler::on, routing::MethodFilter, prelude::*};
///
/// async fn handler() {}
///
/// // Requests to `POST /` will go to `handler`.
/// let app = route("/", on(MethodFilter::POST, handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub fn on<H, B, T>(method: MethodFilter, handler: H) -> OnMethod<IntoService<H, B, T>, EmptyRouter>
where
    H: Handler<B, T>,
{
    OnMethod {
        method,
        svc: handler.into_service(),
        fallback: EmptyRouter::method_not_allowed(),
    }
}

pub(crate) mod sealed {
    #![allow(unreachable_pub, missing_docs, missing_debug_implementations)]

    pub trait HiddentTrait {}
    pub struct Hidden;
    impl HiddentTrait for Hidden {}
}

/// Trait for async functions that can be used to handle requests.
///
/// You shouldn't need to depend on this trait directly. It is automatically
/// implemented to closures of the right types.
///
/// See the [module docs](crate::handler) for more details.
#[async_trait]
pub trait Handler<B, In>: Sized {
    // This seals the trait. We cannot use the regular "sealed super trait"
    // approach due to coherence.
    #[doc(hidden)]
    type Sealed: sealed::HiddentTrait;

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
    /// Note this differs from [`routing::Layered`](crate::routing::Layered)
    /// which adds a middleware to a group of routes.
    ///
    /// # Example
    ///
    /// Adding the [`tower::limit::ConcurrencyLimit`] middleware to a handler
    /// can be done like so:
    ///
    /// ```rust
    /// use axum::prelude::*;
    /// use tower::limit::{ConcurrencyLimitLayer, ConcurrencyLimit};
    ///
    /// async fn handler() { /* ... */ }
    ///
    /// let layered_handler = handler.layer(ConcurrencyLimitLayer::new(64));
    /// let app = route("/", get(layered_handler));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// When adding middleware that might fail its recommended to handle those
    /// errors. See [`Layered::handle_error`] for more details.
    fn layer<L>(self, layer: L) -> Layered<L::Service, In>
    where
        L: Layer<IntoService<Self, B, In>>,
    {
        Layered::new(layer.layer(IntoService::new(self)))
    }

    /// Convert the handler into a [`Service`].
    fn into_service(self) -> IntoService<Self, B, In> {
        IntoService::new(self)
    }
}

#[async_trait]
impl<F, Fut, Res, B> Handler<B, ()> for F
where
    F: FnOnce() -> Fut + Send + Sync,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
{
    type Sealed = sealed::Hidden;

    async fn call(self, _req: Request<B>) -> Response<BoxBody> {
        self().await.into_response().map(box_body)
    }
}

macro_rules! impl_handler {
    () => {
    };

    ( $head:ident, $($tail:ident),* $(,)? ) => {
        #[async_trait]
        #[allow(non_snake_case)]
        impl<F, Fut, B, Res, $head, $($tail,)*> Handler<B, ($head, $($tail,)*)> for F
        where
            F: FnOnce($head, $($tail,)*) -> Fut + Send + Sync,
            Fut: Future<Output = Res> + Send,
            B: Send + 'static,
            Res: IntoResponse,
            B: Send + 'static,
            $head: FromRequest<B> + Send,
            $( $tail: FromRequest<B> + Send,)*
        {
            type Sealed = sealed::Hidden;

            async fn call(self, req: Request<B>) -> Response<BoxBody> {
                let mut req = crate::extract::RequestParts::new(req);

                let $head = match $head::from_request(&mut req).await {
                    Ok(value) => value,
                    Err(rejection) => return rejection.into_response().map(box_body),
                };

                $(
                    let $tail = match $tail::from_request(&mut req).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_response().map(box_body),
                    };
                )*

                let res = self($head, $($tail,)*).await;

                res.into_response().map(crate::body::box_body)
            }
        }

        impl_handler!($($tail,)*);
    };
}

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
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Send,
    S::Error: IntoResponse,
    S::Future: Send,
    ReqBody: Send + 'static,
    ResBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    ResBody::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Sealed = sealed::Hidden;

    async fn call(self, req: Request<ReqBody>) -> Response<BoxBody> {
        match self
            .svc
            .oneshot(req)
            .await
            .map_err(IntoResponse::into_response)
        {
            Ok(res) => res.map(box_body),
            Err(res) => res.map(box_body),
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

    /// Create a new [`Layered`] handler where errors will be handled using the
    /// given closure.
    ///
    /// This is used to convert errors to responses rather than simply
    /// terminating the connection.
    ///
    /// It works similarly to [`routing::RoutingDsl::handle_error`]. See that for more details.
    ///
    /// [`routing::RoutingDsl::handle_error`]: crate::routing::RoutingDsl::handle_error
    pub fn handle_error<F, ReqBody, ResBody, Res, E>(
        self,
        f: F,
    ) -> Layered<HandleError<S, F, ReqBody, HandleErrorFromRouter>, T>
    where
        S: Service<Request<ReqBody>, Response = Response<ResBody>>,
        F: FnOnce(S::Error) -> Result<Res, E>,
        Res: IntoResponse,
    {
        let svc = HandleError::new(self.svc, f);
        Layered::new(svc)
    }
}

/// An adapter that makes a [`Handler`] into a [`Service`].
///
/// Created with [`Handler::into_service`].
pub struct IntoService<H, B, T> {
    handler: H,
    _marker: PhantomData<fn() -> (B, T)>,
}

impl<H, B, T> IntoService<H, B, T> {
    fn new(handler: H) -> Self {
        Self {
            handler,
            _marker: PhantomData,
        }
    }
}

impl<H, B, T> fmt::Debug for IntoService<H, B, T>
where
    H: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoService")
            .field("handler", &self.handler)
            .finish()
    }
}

impl<H, B, T> Clone for IntoService<H, B, T>
where
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            _marker: PhantomData,
        }
    }
}

impl<H, T, B> Service<Request<B>> for IntoService<H, B, T>
where
    H: Handler<B, T> + Clone + Send + 'static,
    B: Send + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = future::IntoServiceFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // `IntoService` can only be constructed from async functions which are always ready, or from
        // `Layered` which bufferes in `<Layered as Handler>::call` and is therefore also always
        // ready.
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let handler = self.handler.clone();
        let future = Box::pin(async move {
            let res = Handler::call(handler, req).await;
            Ok(res)
        });
        future::IntoServiceFuture { future }
    }
}

/// A handler [`Service`] that accepts requests based on a [`MethodFilter`] and
/// allows chaining additional handlers.
#[derive(Debug, Clone, Copy)]
pub struct OnMethod<S, F> {
    pub(crate) method: MethodFilter,
    pub(crate) svc: S,
    pub(crate) fallback: F,
}

impl<S, F> OnMethod<S, F> {
    /// Chain an additional handler that will accept all requests regardless of
    /// its HTTP method.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn any<H, B, T>(self, handler: H) -> OnMethod<IntoService<H, B, T>, Self>
    where
        H: Handler<B, T>,
    {
        self.on(MethodFilter::all(), handler)
    }

    /// Chain an additional handler that will only accept `CONNECT` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn connect<H, B, T>(self, handler: H) -> OnMethod<IntoService<H, B, T>, Self>
    where
        H: Handler<B, T>,
    {
        self.on(MethodFilter::CONNECT, handler)
    }

    /// Chain an additional handler that will only accept `DELETE` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn delete<H, B, T>(self, handler: H) -> OnMethod<IntoService<H, B, T>, Self>
    where
        H: Handler<B, T>,
    {
        self.on(MethodFilter::DELETE, handler)
    }

    /// Chain an additional handler that will only accept `GET` requests.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::prelude::*;
    ///
    /// async fn handler() {}
    ///
    /// async fn other_handler() {}
    ///
    /// // Requests to `GET /` will go to `handler` and `POST /` will go to
    /// // `other_handler`.
    /// let app = route("/", post(handler).get(other_handler));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn get<H, B, T>(self, handler: H) -> OnMethod<IntoService<H, B, T>, Self>
    where
        H: Handler<B, T>,
    {
        self.on(MethodFilter::GET, handler)
    }

    /// Chain an additional handler that will only accept `HEAD` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn head<H, B, T>(self, handler: H) -> OnMethod<IntoService<H, B, T>, Self>
    where
        H: Handler<B, T>,
    {
        self.on(MethodFilter::HEAD, handler)
    }

    /// Chain an additional handler that will only accept `OPTIONS` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn options<H, B, T>(self, handler: H) -> OnMethod<IntoService<H, B, T>, Self>
    where
        H: Handler<B, T>,
    {
        self.on(MethodFilter::OPTIONS, handler)
    }

    /// Chain an additional handler that will only accept `PATCH` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn patch<H, B, T>(self, handler: H) -> OnMethod<IntoService<H, B, T>, Self>
    where
        H: Handler<B, T>,
    {
        self.on(MethodFilter::PATCH, handler)
    }

    /// Chain an additional handler that will only accept `POST` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn post<H, B, T>(self, handler: H) -> OnMethod<IntoService<H, B, T>, Self>
    where
        H: Handler<B, T>,
    {
        self.on(MethodFilter::POST, handler)
    }

    /// Chain an additional handler that will only accept `PUT` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn put<H, B, T>(self, handler: H) -> OnMethod<IntoService<H, B, T>, Self>
    where
        H: Handler<B, T>,
    {
        self.on(MethodFilter::PUT, handler)
    }

    /// Chain an additional handler that will only accept `TRACE` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn trace<H, B, T>(self, handler: H) -> OnMethod<IntoService<H, B, T>, Self>
    where
        H: Handler<B, T>,
    {
        self.on(MethodFilter::TRACE, handler)
    }

    /// Chain an additional handler that will accept requests matching the given
    /// `MethodFilter`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::{routing::MethodFilter, prelude::*};
    ///
    /// async fn handler() {}
    ///
    /// async fn other_handler() {}
    ///
    /// // Requests to `GET /` will go to `handler` and `DELETE /` will go to
    /// // `other_handler`
    /// let app = route("/", get(handler).on(MethodFilter::DELETE, other_handler));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn on<H, B, T>(
        self,
        method: MethodFilter,
        handler: H,
    ) -> OnMethod<IntoService<H, B, T>, Self>
    where
        H: Handler<B, T>,
    {
        OnMethod {
            method,
            svc: handler.into_service(),
            fallback: self,
        }
    }
}

impl<S, F, B> Service<Request<B>> for OnMethod<S, F>
where
    S: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible> + Clone,
    F: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible> + Clone,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = future::OnMethodFuture<S, F, B>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let f = if self.method.matches(req.method()) {
            let fut = self.svc.clone().oneshot(req);
            RouteFuture::a(fut)
        } else {
            let fut = self.fallback.clone().oneshot(req);
            RouteFuture::b(fut)
        };

        future::OnMethodFuture { inner: f }
    }
}
