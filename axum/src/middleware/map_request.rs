#![allow(missing_docs, unused_imports)]

use crate::response::{IntoResponse, Response};
use axum_core::extract::{FromRequest, FromRequestParts};
use futures_util::future::BoxFuture;
use http::Request;
use std::{
    any::type_name,
    convert::Infallible,
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{util::BoxCloneService, ServiceBuilder};
use tower_layer::Layer;
use tower_service::Service;

// TODO(david): naming
pub trait IntoResult<T> {
    fn into_result(self) -> Result<T, Response>;
}

impl<T, E> IntoResult<T> for Result<T, E>
where
    E: IntoResponse,
{
    fn into_result(self) -> Result<T, Response> {
        self.map_err(IntoResponse::into_response)
    }
}

impl<B> IntoResult<Request<B>> for Request<B> {
    fn into_result(self) -> Result<Request<B>, Response> {
        Ok(self)
    }
}

pub fn map_request<F, T>(f: F) -> MapRequestLayer<F, (), T> {
    map_request_with_state((), f)
}

pub fn map_request_with_state<F, S, T>(state: S, f: F) -> MapRequestLayer<F, S, T> {
    map_request_with_state_arc(Arc::new(state), f)
}

pub fn map_request_with_state_arc<F, S, T>(state: Arc<S>, f: F) -> MapRequestLayer<F, S, T> {
    MapRequestLayer {
        f,
        state,
        _extractor: PhantomData,
    }
}

pub struct MapRequestLayer<F, S, T> {
    f: F,
    state: Arc<S>,
    _extractor: PhantomData<fn() -> T>,
}

impl<F, S, T> Clone for MapRequestLayer<F, S, T>
where
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            f: self.f.clone(),
            state: Arc::clone(&self.state),
            _extractor: self._extractor,
        }
    }
}

impl<S, I, F, T> Layer<I> for MapRequestLayer<F, S, T>
where
    F: Clone,
{
    type Service = MapRequest<F, S, I, T>;

    fn layer(&self, inner: I) -> Self::Service {
        MapRequest {
            f: self.f.clone(),
            state: Arc::clone(&self.state),
            inner,
            _extractor: PhantomData,
        }
    }
}

impl<F, S, T> fmt::Debug for MapRequestLayer<F, S, T>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapRequestLayer")
            // Write out the type name, without quoting it as `&type_name::<F>()` would
            .field("f", &format_args!("{}", type_name::<F>()))
            .field("state", &self.state)
            .finish()
    }
}

pub struct MapRequest<F, S, I, T> {
    f: F,
    inner: I,
    state: Arc<S>,
    _extractor: PhantomData<fn() -> T>,
}

impl<F, S, I, T> Clone for MapRequest<F, S, I, T>
where
    F: Clone,
    I: Clone,
{
    fn clone(&self) -> Self {
        Self {
            f: self.f.clone(),
            inner: self.inner.clone(),
            state: Arc::clone(&self.state),
            _extractor: self._extractor,
        }
    }
}

macro_rules! impl_service {
    (
        [$($ty:ident),*], $last:ident
    ) => {
        #[allow(non_snake_case, unused_mut)]
        impl<F, Fut, S, I, B, $($ty,)* $last> Service<Request<B>> for MapRequest<F, S, I, ($($ty,)* $last,)>
        where
            F: FnMut($($ty,)* $last) -> Fut + Clone + Send + 'static,
            $( $ty: FromRequestParts<S> + Send, )*
            $last: FromRequest<S, B> + Send,
            Fut: Future + Send + 'static,
            Fut::Output: IntoResult<Request<B>> + Send + 'static,
            I: Service<Request<B>, Error = Infallible>
                + Clone
                + Send
                + 'static,
            I::Response: IntoResponse,
            I::Future: Send + 'static,
            B: Send + 'static,
            S: Send + Sync + 'static,
        {
            type Response = Response;
            type Error = Infallible;
            type Future = ResponseFuture;

            fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                self.inner.poll_ready(cx)
            }

            fn call(&mut self, req: Request<B>) -> Self::Future {
                let not_ready_inner = self.inner.clone();
                let mut ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);

                let mut f = self.f.clone();
                let state = Arc::clone(&self.state);

                let future = Box::pin(async move {
                    let (mut parts, body) = req.into_parts();

                    $(
                        let $ty = match $ty::from_request_parts(&mut parts, &state).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_response(),
                        };
                    )*

                    let req = Request::from_parts(parts, body);

                    let $last = match $last::from_request(req, &state).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_response(),
                    };

                    match f($($ty,)* $last).await.into_result() {
                        Ok(req) => {
                            ready_inner.call(req).await.into_response()
                        }
                        Err(res) => {
                            res
                        }
                    }
                });

                ResponseFuture {
                    inner: future
                }
            }
        }
    };
}

impl_service!([], T1);
impl_service!([T1], T2);
impl_service!([T1, T2], T3);
impl_service!([T1, T2, T3], T4);
impl_service!([T1, T2, T3, T4], T5);
impl_service!([T1, T2, T3, T4, T5], T6);
impl_service!([T1, T2, T3, T4, T5, T6], T7);
impl_service!([T1, T2, T3, T4, T5, T6, T7], T8);
impl_service!([T1, T2, T3, T4, T5, T6, T7, T8], T9);
impl_service!([T1, T2, T3, T4, T5, T6, T7, T8, T9], T10);
impl_service!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10], T11);
impl_service!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11], T12);
impl_service!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12], T13);
impl_service!(
    [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13],
    T14
);
impl_service!(
    [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14],
    T15
);
impl_service!(
    [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15],
    T16
);

impl<F, S, I, T> fmt::Debug for MapRequest<F, S, I, T>
where
    S: fmt::Debug,
    I: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapRequest")
            .field("f", &format_args!("{}", type_name::<F>()))
            .field("inner", &self.inner)
            .field("state", &self.state)
            .finish()
    }
}

/// Response future for [`MapRequest`].
pub struct ResponseFuture {
    inner: BoxFuture<'static, Response>,
}

impl Future for ResponseFuture {
    type Output = Result<Response, Infallible>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner.as_mut().poll(cx).map(Ok)
    }
}

impl fmt::Debug for ResponseFuture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseFuture").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{routing::get, test_helpers::TestClient, Router};
    use http::{HeaderMap, StatusCode};

    #[tokio::test]
    async fn works() {
        async fn add_header<B>(mut req: Request<B>) -> Request<B> {
            req.headers_mut().insert("x-foo", "foo".parse().unwrap());
            req
        }

        async fn handler(headers: HeaderMap) -> Response {
            headers["x-foo"]
                .to_str()
                .unwrap()
                .to_owned()
                .into_response()
        }

        let app = Router::new()
            .route("/", get(handler))
            .layer(map_request(add_header));
        let client = TestClient::new(app);

        let res = client.get("/").send().await;

        assert_eq!(res.text().await, "foo");
    }

    #[tokio::test]
    async fn works_for_short_circutting() {
        async fn add_header<B>(_req: Request<B>) -> Result<Request<B>, (StatusCode, &'static str)> {
            Err((StatusCode::INTERNAL_SERVER_ERROR, "something went wrong"))
        }

        async fn handler(_headers: HeaderMap) -> Response {
            unreachable!()
        }

        let app = Router::new()
            .route("/", get(handler))
            .layer(map_request(add_header));
        let client = TestClient::new(app);

        let res = client.get("/").send().await;

        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(res.text().await, "something went wrong");
    }
}
