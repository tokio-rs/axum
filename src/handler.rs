use crate::{
    body::{Body, BoxBody},
    extract::FromRequest,
    response::IntoResponse,
    routing::{BoxResponseBody, EmptyRouter, MethodFilter, RouteFuture},
    service::HandleError,
};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::future;
use http::{Request, Response};
use std::{
    convert::Infallible,
    future::Future,
    marker::PhantomData,
    task::{Context, Poll},
};
use tower::{BoxError, Layer, Service, ServiceExt};

pub fn any<H, T>(handler: H) -> OnMethod<IntoService<H, T>, EmptyRouter>
where
    H: Handler<T>,
{
    on(MethodFilter::Any, handler)
}

pub fn connect<H, T>(handler: H) -> OnMethod<IntoService<H, T>, EmptyRouter>
where
    H: Handler<T>,
{
    on(MethodFilter::Connect, handler)
}

pub fn delete<H, T>(handler: H) -> OnMethod<IntoService<H, T>, EmptyRouter>
where
    H: Handler<T>,
{
    on(MethodFilter::Delete, handler)
}

pub fn get<H, T>(handler: H) -> OnMethod<IntoService<H, T>, EmptyRouter>
where
    H: Handler<T>,
{
    on(MethodFilter::Get, handler)
}

pub fn head<H, T>(handler: H) -> OnMethod<IntoService<H, T>, EmptyRouter>
where
    H: Handler<T>,
{
    on(MethodFilter::Head, handler)
}

pub fn options<H, T>(handler: H) -> OnMethod<IntoService<H, T>, EmptyRouter>
where
    H: Handler<T>,
{
    on(MethodFilter::Options, handler)
}

pub fn patch<H, T>(handler: H) -> OnMethod<IntoService<H, T>, EmptyRouter>
where
    H: Handler<T>,
{
    on(MethodFilter::Patch, handler)
}

pub fn post<H, T>(handler: H) -> OnMethod<IntoService<H, T>, EmptyRouter>
where
    H: Handler<T>,
{
    on(MethodFilter::Post, handler)
}

pub fn put<H, T>(handler: H) -> OnMethod<IntoService<H, T>, EmptyRouter>
where
    H: Handler<T>,
{
    on(MethodFilter::Put, handler)
}

pub fn trace<H, T>(handler: H) -> OnMethod<IntoService<H, T>, EmptyRouter>
where
    H: Handler<T>,
{
    on(MethodFilter::Trace, handler)
}

pub fn on<H, T>(method: MethodFilter, handler: H) -> OnMethod<IntoService<H, T>, EmptyRouter>
where
    H: Handler<T>,
{
    OnMethod {
        method,
        svc: handler.into_service(),
        fallback: EmptyRouter,
    }
}

mod sealed {
    #![allow(unreachable_pub)]

    pub trait HiddentTrait {}
    pub struct Hidden;
    impl HiddentTrait for Hidden {}
}

#[async_trait]
pub trait Handler<In>: Sized {
    // This seals the trait. We cannot use the regular "sealed super trait" approach
    // due to coherence.
    #[doc(hidden)]
    type Sealed: sealed::HiddentTrait;

    async fn call(self, req: Request<Body>) -> Response<BoxBody>;

    fn layer<L>(self, layer: L) -> Layered<L::Service, In>
    where
        L: Layer<IntoService<Self, In>>,
    {
        Layered::new(layer.layer(IntoService::new(self)))
    }

    fn into_service(self) -> IntoService<Self, In> {
        IntoService::new(self)
    }
}

#[async_trait]
impl<F, Fut, Res> Handler<()> for F
where
    F: FnOnce(Request<Body>) -> Fut + Send + Sync,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
{
    type Sealed = sealed::Hidden;

    async fn call(self, req: Request<Body>) -> Response<BoxBody> {
        self(req).await.into_response().map(BoxBody::new)
    }
}

macro_rules! impl_handler {
    () => {};

    ( $head:ident, $($tail:ident),* $(,)? ) => {
        #[async_trait]
        #[allow(non_snake_case)]
        impl<F, Fut, Res, $head, $($tail,)*> Handler<($head, $($tail,)*)> for F
        where
            F: FnOnce(Request<Body>, $head, $($tail,)*) -> Fut + Send + Sync,
            Fut: Future<Output = Res> + Send,
            Res: IntoResponse,
            $head: FromRequest + Send,
            $( $tail: FromRequest + Send, )*
        {
            type Sealed = sealed::Hidden;

            async fn call(self, mut req: Request<Body>) -> Response<BoxBody> {
                let $head = match $head::from_request(&mut req).await {
                    Ok(value) => value,
                    Err(rejection) => return rejection.into_response().map(BoxBody::new),
                };

                $(
                    let $tail = match $tail::from_request(&mut req).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_response().map(BoxBody::new),
                    };
                )*

                let res = self(req, $head, $($tail,)*).await;

                res.into_response().map(BoxBody::new)
            }
        }

        impl_handler!($($tail,)*);
    };
}

impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);

pub struct Layered<S, T> {
    svc: S,
    _input: PhantomData<fn() -> T>,
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
impl<S, T, B> Handler<T> for Layered<S, T>
where
    S: Service<Request<Body>, Response = Response<B>> + Send,
    // S::Response: IntoResponse,
    S::Error: IntoResponse,
    S::Future: Send,
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Sealed = sealed::Hidden;

    async fn call(self, req: Request<Body>) -> Response<BoxBody> {
        match self
            .svc
            .oneshot(req)
            .await
            .map_err(IntoResponse::into_response)
        {
            Ok(res) => res.map(BoxBody::new),
            Err(res) => res.map(BoxBody::new),
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

    pub fn handle_error<F, B, Res>(self, f: F) -> Layered<HandleError<S, F>, T>
    where
        S: Service<Request<Body>, Response = Response<B>>,
        F: FnOnce(S::Error) -> Res,
        Res: IntoResponse,
    {
        let svc = HandleError::new(self.svc, f);
        Layered::new(svc)
    }
}

pub struct IntoService<H, T> {
    handler: H,
    _marker: PhantomData<fn() -> T>,
}

impl<H, T> IntoService<H, T> {
    fn new(handler: H) -> Self {
        Self {
            handler,
            _marker: PhantomData,
        }
    }
}

impl<H, T> Clone for IntoService<H, T>
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

impl<H, T> Service<Request<Body>> for IntoService<H, T>
where
    H: Handler<T> + Clone + Send + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = IntoServiceFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // `IntoService` can only be constructed from async functions which are always ready, or from
        // `Layered` which bufferes in `<Layered as Handler>::call` and is therefore also always
        // ready.
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let handler = self.handler.clone();
        let future = Box::pin(async move {
            let res = Handler::call(handler, req).await;
            Ok(res)
        });
        IntoServiceFuture(future)
    }
}

opaque_future! {
    pub type IntoServiceFuture =
        future::BoxFuture<'static, Result<Response<BoxBody>, Infallible>>;
}

#[derive(Clone)]
pub struct OnMethod<S, F> {
    pub(crate) method: MethodFilter,
    pub(crate) svc: S,
    pub(crate) fallback: F,
}

impl<S, F> OnMethod<S, F> {
    pub fn any<H, T>(self, handler: H) -> OnMethod<IntoService<H, T>, Self>
    where
        H: Handler<T>,
    {
        self.on(MethodFilter::Any, handler)
    }

    pub fn connect<H, T>(self, handler: H) -> OnMethod<IntoService<H, T>, Self>
    where
        H: Handler<T>,
    {
        self.on(MethodFilter::Connect, handler)
    }

    pub fn delete<H, T>(self, handler: H) -> OnMethod<IntoService<H, T>, Self>
    where
        H: Handler<T>,
    {
        self.on(MethodFilter::Delete, handler)
    }

    pub fn get<H, T>(self, handler: H) -> OnMethod<IntoService<H, T>, Self>
    where
        H: Handler<T>,
    {
        self.on(MethodFilter::Get, handler)
    }

    pub fn head<H, T>(self, handler: H) -> OnMethod<IntoService<H, T>, Self>
    where
        H: Handler<T>,
    {
        self.on(MethodFilter::Head, handler)
    }

    pub fn options<H, T>(self, handler: H) -> OnMethod<IntoService<H, T>, Self>
    where
        H: Handler<T>,
    {
        self.on(MethodFilter::Options, handler)
    }

    pub fn patch<H, T>(self, handler: H) -> OnMethod<IntoService<H, T>, Self>
    where
        H: Handler<T>,
    {
        self.on(MethodFilter::Patch, handler)
    }

    pub fn post<H, T>(self, handler: H) -> OnMethod<IntoService<H, T>, Self>
    where
        H: Handler<T>,
    {
        self.on(MethodFilter::Post, handler)
    }

    pub fn put<H, T>(self, handler: H) -> OnMethod<IntoService<H, T>, Self>
    where
        H: Handler<T>,
    {
        self.on(MethodFilter::Put, handler)
    }

    pub fn trace<H, T>(self, handler: H) -> OnMethod<IntoService<H, T>, Self>
    where
        H: Handler<T>,
    {
        self.on(MethodFilter::Trace, handler)
    }

    pub fn on<H, T>(self, method: MethodFilter, handler: H) -> OnMethod<IntoService<H, T>, Self>
    where
        H: Handler<T>,
    {
        OnMethod {
            method,
            svc: handler.into_service(),
            fallback: self,
        }
    }
}

impl<S, F, SB, FB> Service<Request<Body>> for OnMethod<S, F>
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

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let f = if self.method.matches(req.method()) {
            let response_future = self.svc.clone().oneshot(req);
            future::Either::Left(BoxResponseBody(response_future))
        } else {
            let response_future = self.fallback.clone().oneshot(req);
            future::Either::Right(BoxResponseBody(response_future))
        };
        RouteFuture(f)
    }
}
