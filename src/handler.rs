use crate::{body::Body, error::Error, extract::FromRequest, response::IntoResponse};
use async_trait::async_trait;
use futures_util::future;
use http::{Request, Response};
use std::{
    future::Future,
    marker::PhantomData,
    task::{Context, Poll},
};
use tower::{BoxError, Layer, Service, ServiceExt};

mod sealed {
    pub trait HiddentTrait {}
    pub struct Hidden;
    impl HiddentTrait for Hidden {}
}

#[async_trait]
pub trait Handler<B, In>: Sized {
    type Response: IntoResponse<B>;

    // This seals the trait. We cannot use the regular "sealed super trait" approach
    // due to coherence.
    #[doc(hidden)]
    type Sealed: sealed::HiddentTrait;

    async fn call(self, req: Request<Body>) -> Result<Response<B>, Error>;

    fn layer<L>(self, layer: L) -> Layered<L::Service, In>
    where
        L: Layer<HandlerSvc<Self, B, In>>,
    {
        Layered::new(layer.layer(HandlerSvc::new(self)))
    }
}

#[async_trait]
impl<F, Fut, B, Res> Handler<B, ()> for F
where
    F: Fn(Request<Body>) -> Fut + Send + Sync,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse<B>,
{
    type Response = Res;

    type Sealed = sealed::Hidden;

    async fn call(self, req: Request<Body>) -> Result<Response<B>, Error> {
        self(req).await.into_response()
    }
}

macro_rules! impl_handler {
    () => {};

    ( $head:ident, $($tail:ident),* $(,)? ) => {
        #[async_trait]
        #[allow(non_snake_case)]
        impl<F, Fut, B, Res, $head, $($tail,)*> Handler<B, ($head, $($tail,)*)> for F
        where
            F: Fn(Request<Body>, $head, $($tail,)*) -> Fut + Send + Sync,
            Fut: Future<Output = Res> + Send,
            Res: IntoResponse<B>,
            $head: FromRequest + Send,
            $( $tail: FromRequest + Send, )*
        {
            type Response = Res;

            type Sealed = sealed::Hidden;

            async fn call(self, mut req: Request<Body>) -> Result<Response<B>, Error> {
                let $head = $head::from_request(&mut req).await?;
                $(
                    let $tail = $tail::from_request(&mut req).await?;
                )*
                let res = self(req, $head, $($tail,)*).await;
                res.into_response()
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
impl<S, B, T> Handler<B, T> for Layered<S, T>
where
    S: Service<Request<Body>, Response = Response<B>> + Send,
    S::Error: Into<BoxError>,
    S::Future: Send,
{
    type Response = S::Response;

    type Sealed = sealed::Hidden;

    async fn call(self, req: Request<Body>) -> Result<Self::Response, Error> {
        self.svc
            .oneshot(req)
            .await
            .map_err(|err| Error::Service(err.into()))
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

pub struct HandlerSvc<H, B, T> {
    handler: H,
    _input: PhantomData<fn() -> (B, T)>,
}

impl<H, B, T> HandlerSvc<H, B, T> {
    pub(crate) fn new(handler: H) -> Self {
        Self {
            handler,
            _input: PhantomData,
        }
    }
}

impl<H, B, T> Clone for HandlerSvc<H, B, T>
where
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            _input: PhantomData,
        }
    }
}

impl<H, B, T> Service<Request<Body>> for HandlerSvc<H, B, T>
where
    H: Handler<B, T> + Clone + Send + 'static,
    H::Response: 'static,
{
    type Response = Response<B>;
    type Error = Error;
    type Future = future::BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // HandlerSvc can only be constructed from async functions which are always ready, or from
        // `Layered` which bufferes in `<Layered as Handler>::call` and is therefore also always
        // ready.
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let handler = self.handler.clone();
        Box::pin(async move {
            let res = Handler::call(handler, req).await?.into_response()?;
            Ok(res)
        })
    }
}
