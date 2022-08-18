use axum_core::{
    extract::{FromRequest, FromRequestParts},
    response::{IntoResponse, Response},
};
use http::Request;
use hyper::Body;
use std::{future::Future, pin::Pin, sync::Arc};

pub trait Handler<T, S = (), B = Body>: Clone + Send + Sized + 'static {
    /// The type of future calling this handler returns.
    type Future: Future<Output = Response> + Send + 'static;

    /// Call the handler with the given request.
    fn call(self, state: Arc<S>, req: Request<B>) -> Self::Future;
}

impl<F, Fut, Res, S, B, M> Handler<(M,), S, B> for F
where
    F: FnOnce() -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

    fn call(self, _state: Arc<S>, _req: Request<B>) -> Self::Future {
        Box::pin(async move { self().await.into_response() })
    }
}

#[allow(non_snake_case)]
impl<F, Fut, S, B, Res, T1, M> Handler<(M, T1), S, B> for F
where
    F: FnOnce(T1) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    B: Send + 'static,
    S: Send + Sync + 'static,
    Res: IntoResponse,
    T1: FromRequest<S, B, M> + Send,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    fn call(self, state: Arc<S>, req: Request<B>) -> Self::Future {
        Box::pin(async move {
            let T1 = match T1::from_request_once(req, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let res = self(T1).await;
            res.into_response()
        })
    }
}

#[allow(non_snake_case)]
impl<F, Fut, S, B, Res, T1, T2, M> Handler<(M, T1, T2), S, B> for F
where
    F: FnOnce(T1, T2) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    B: Send + 'static,
    S: Send + Sync + 'static,
    Res: IntoResponse,
    T1: FromRequestParts<S, B> + Send,
    T2: FromRequestParts<S, B> + Send,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    fn call(self, state: Arc<S>, mut req: Request<B>) -> Self::Future {
        Box::pin(async move {
            let T1 = match T1::from_request_mut(&mut req, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request_once(req, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let res = self(T1, T2).await;
            res.into_response()
        })
    }
}

#[allow(non_snake_case)]
impl<F, Fut, S, B, Res, T1, T2, T3, M> Handler<(M, T1, T2, T3), S, B> for F
where
    F: FnOnce(T1, T2, T3) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    B: Send + 'static,
    S: Send + Sync + 'static,
    Res: IntoResponse,
    T1: FromRequestParts<S, B> + Send,
    T2: FromRequestParts<S, B> + Send,
    T3: FromRequestParts<S, B> + Send,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    fn call(self, state: Arc<S>, mut req: Request<B>) -> Self::Future {
        Box::pin(async move {
            let T1 = match T1::from_request_mut(&mut req, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request_mut(&mut req, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T3 = match T3::from_request_once(req, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let res = self(T1, T2, T3).await;
            res.into_response()
        })
    }
}

#[allow(non_snake_case)]
impl<F, Fut, S, B, Res, T1, T2, T3, T4, M> Handler<(M, T1, T2, T3, T4), S, B> for F
where
    F: FnOnce(T1, T2, T3, T4) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    B: Send + 'static,
    S: Send + Sync + 'static,
    Res: IntoResponse,
    T1: FromRequestParts<S, B> + Send,
    T2: FromRequestParts<S, B> + Send,
    T3: FromRequestParts<S, B> + Send,
    T4: FromRequestParts<S, B> + Send,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    fn call(self, state: Arc<S>, mut req: Request<B>) -> Self::Future {
        Box::pin(async move {
            let T1 = match T1::from_request_mut(&mut req, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request_mut(&mut req, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T3 = match T3::from_request_mut(&mut req, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T4 = match T4::from_request_once(req, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let res = self(T1, T2, T3, T4).await;
            res.into_response()
        })
    }
}
