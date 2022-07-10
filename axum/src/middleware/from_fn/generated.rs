// this file is machine generated. Don't edit it!

use super::*;
use axum_core::extract::{Mut, Once};

#[allow(non_snake_case, unused_mut)]
impl<F, Fut, Out, S, B, ResBody, T1> Service<Request<B>> for FromFn<F, S, (T1,)>
where
    F: FnMut(T1, Next<B>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(T1, next).await.into_response()
        });
        ResponseFuture { inner: future }
    }
}
#[allow(non_snake_case, unused_mut)]
impl<F, Fut, Out, S, B, ResBody, T1, T2> Service<Request<B>> for FromFn<F, S, (T1, T2)>
where
    F: FnMut(T1, T2, Next<B>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T2 = match T2::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(T1, T2, next).await.into_response()
        });
        ResponseFuture { inner: future }
    }
}
#[allow(non_snake_case, unused_mut)]
impl<F, Fut, Out, S, B, ResBody, T1, T2, T3> Service<Request<B>> for FromFn<F, S, (T1, T2, T3)>
where
    F: FnMut(T1, T2, T3, Next<B>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T3 = match T3::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(T1, T2, T3, next).await.into_response()
        });
        ResponseFuture { inner: future }
    }
}
#[allow(non_snake_case, unused_mut)]
impl<F, Fut, Out, S, B, ResBody, T1, T2, T3, T4> Service<Request<B>>
    for FromFn<F, S, (T1, T2, T3, T4)>
where
    F: FnMut(T1, T2, T3, T4, Next<B>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T3 = match T3::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T4 = match T4::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(T1, T2, T3, T4, next).await.into_response()
        });
        ResponseFuture { inner: future }
    }
}
#[allow(non_snake_case, unused_mut)]
impl<F, Fut, Out, S, B, ResBody, T1, T2, T3, T4, T5> Service<Request<B>>
    for FromFn<F, S, (T1, T2, T3, T4, T5)>
where
    F: FnMut(T1, T2, T3, T4, T5, Next<B>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T3 = match T3::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T4 = match T4::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T5 = match T5::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(T1, T2, T3, T4, T5, next).await.into_response()
        });
        ResponseFuture { inner: future }
    }
}
#[allow(non_snake_case, unused_mut)]
impl<F, Fut, Out, S, B, ResBody, T1, T2, T3, T4, T5, T6> Service<Request<B>>
    for FromFn<F, S, (T1, T2, T3, T4, T5, T6)>
where
    F: FnMut(T1, T2, T3, T4, T5, T6, Next<B>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T3 = match T3::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T4 = match T4::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T5 = match T5::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T6 = match T6::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(T1, T2, T3, T4, T5, T6, next).await.into_response()
        });
        ResponseFuture { inner: future }
    }
}
#[allow(non_snake_case, unused_mut)]
impl<F, Fut, Out, S, B, ResBody, T1, T2, T3, T4, T5, T6, T7> Service<Request<B>>
    for FromFn<F, S, (T1, T2, T3, T4, T5, T6, T7)>
where
    F: FnMut(T1, T2, T3, T4, T5, T6, T7, Next<B>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T3 = match T3::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T4 = match T4::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T5 = match T5::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T6 = match T6::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T7 = match T7::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(T1, T2, T3, T4, T5, T6, T7, next).await.into_response()
        });
        ResponseFuture { inner: future }
    }
}
#[allow(non_snake_case, unused_mut)]
impl<F, Fut, Out, S, B, ResBody, T1, T2, T3, T4, T5, T6, T7, T8> Service<Request<B>>
    for FromFn<F, S, (T1, T2, T3, T4, T5, T6, T7, T8)>
where
    F: FnMut(T1, T2, T3, T4, T5, T6, T7, T8, Next<B>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T3 = match T3::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T4 = match T4::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T5 = match T5::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T6 = match T6::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T7 = match T7::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T8 = match T8::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(T1, T2, T3, T4, T5, T6, T7, T8, next)
                .await
                .into_response()
        });
        ResponseFuture { inner: future }
    }
}
#[allow(non_snake_case, unused_mut)]
impl<F, Fut, Out, S, B, ResBody, T1, T2, T3, T4, T5, T6, T7, T8, T9> Service<Request<B>>
    for FromFn<F, S, (T1, T2, T3, T4, T5, T6, T7, T8, T9)>
where
    F: FnMut(T1, T2, T3, T4, T5, T6, T7, T8, T9, Next<B>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T3 = match T3::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T4 = match T4::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T5 = match T5::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T6 = match T6::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T7 = match T7::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T8 = match T8::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T9 = match T9::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(T1, T2, T3, T4, T5, T6, T7, T8, T9, next)
                .await
                .into_response()
        });
        ResponseFuture { inner: future }
    }
}
#[allow(non_snake_case, unused_mut)]
impl<F, Fut, Out, S, B, ResBody, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10> Service<Request<B>>
    for FromFn<F, S, (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10)>
where
    F: FnMut(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, Next<B>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Mut, B> + Send,
    T10: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T3 = match T3::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T4 = match T4::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T5 = match T5::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T6 = match T6::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T7 = match T7::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T8 = match T8::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T9 = match T9::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T10 = match T10::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, next)
                .await
                .into_response()
        });
        ResponseFuture { inner: future }
    }
}
#[allow(non_snake_case, unused_mut)]
impl<F, Fut, Out, S, B, ResBody, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11> Service<Request<B>>
    for FromFn<F, S, (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11)>
where
    F: FnMut(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, Next<B>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Mut, B> + Send,
    T10: FromRequest<Mut, B> + Send,
    T11: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T3 = match T3::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T4 = match T4::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T5 = match T5::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T6 = match T6::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T7 = match T7::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T8 = match T8::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T9 = match T9::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T10 = match T10::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T11 = match T11::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, next)
                .await
                .into_response()
        });
        ResponseFuture { inner: future }
    }
}
#[allow(non_snake_case, unused_mut)]
impl<F, Fut, Out, S, B, ResBody, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12>
    Service<Request<B>> for FromFn<F, S, (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12)>
where
    F: FnMut(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, Next<B>) -> Fut
        + Clone
        + Send
        + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Mut, B> + Send,
    T10: FromRequest<Mut, B> + Send,
    T11: FromRequest<Mut, B> + Send,
    T12: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T3 = match T3::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T4 = match T4::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T5 = match T5::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T6 = match T6::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T7 = match T7::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T8 = match T8::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T9 = match T9::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T10 = match T10::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T11 = match T11::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T12 = match T12::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, next)
                .await
                .into_response()
        });
        ResponseFuture { inner: future }
    }
}
#[allow(non_snake_case, unused_mut)]
impl<F, Fut, Out, S, B, ResBody, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13>
    Service<Request<B>> for FromFn<F, S, (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13)>
where
    F: FnMut(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, Next<B>) -> Fut
        + Clone
        + Send
        + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Mut, B> + Send,
    T10: FromRequest<Mut, B> + Send,
    T11: FromRequest<Mut, B> + Send,
    T12: FromRequest<Mut, B> + Send,
    T13: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T3 = match T3::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T4 = match T4::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T5 = match T5::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T6 = match T6::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T7 = match T7::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T8 = match T8::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T9 = match T9::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T10 = match T10::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T11 = match T11::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T12 = match T12::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T13 = match T13::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, next)
                .await
                .into_response()
        });
        ResponseFuture { inner: future }
    }
}
#[allow(non_snake_case, unused_mut)]
impl<F, Fut, Out, S, B, ResBody, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14>
    Service<Request<B>>
    for FromFn<F, S, (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14)>
where
    F: FnMut(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, Next<B>) -> Fut
        + Clone
        + Send
        + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Mut, B> + Send,
    T10: FromRequest<Mut, B> + Send,
    T11: FromRequest<Mut, B> + Send,
    T12: FromRequest<Mut, B> + Send,
    T13: FromRequest<Mut, B> + Send,
    T14: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T3 = match T3::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T4 = match T4::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T5 = match T5::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T6 = match T6::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T7 = match T7::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T8 = match T8::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T9 = match T9::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T10 = match T10::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T11 = match T11::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T12 = match T12::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T13 = match T13::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T14 = match T14::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(
                T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, next,
            )
            .await
            .into_response()
        });
        ResponseFuture { inner: future }
    }
}
#[allow(non_snake_case, unused_mut)]
impl<
        F,
        Fut,
        Out,
        S,
        B,
        ResBody,
        T1,
        T2,
        T3,
        T4,
        T5,
        T6,
        T7,
        T8,
        T9,
        T10,
        T11,
        T12,
        T13,
        T14,
        T15,
    > Service<Request<B>>
    for FromFn<
        F,
        S,
        (
            T1,
            T2,
            T3,
            T4,
            T5,
            T6,
            T7,
            T8,
            T9,
            T10,
            T11,
            T12,
            T13,
            T14,
            T15,
        ),
    >
where
    F: FnMut(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, Next<B>) -> Fut
        + Clone
        + Send
        + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Mut, B> + Send,
    T10: FromRequest<Mut, B> + Send,
    T11: FromRequest<Mut, B> + Send,
    T12: FromRequest<Mut, B> + Send,
    T13: FromRequest<Mut, B> + Send,
    T14: FromRequest<Mut, B> + Send,
    T15: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T3 = match T3::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T4 = match T4::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T5 = match T5::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T6 = match T6::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T7 = match T7::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T8 = match T8::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T9 = match T9::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T10 = match T10::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T11 = match T11::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T12 = match T12::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T13 = match T13::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T14 = match T14::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T15 = match T15::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(
                T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, next,
            )
            .await
            .into_response()
        });
        ResponseFuture { inner: future }
    }
}
#[allow(non_snake_case, unused_mut)]
impl<
        F,
        Fut,
        Out,
        S,
        B,
        ResBody,
        T1,
        T2,
        T3,
        T4,
        T5,
        T6,
        T7,
        T8,
        T9,
        T10,
        T11,
        T12,
        T13,
        T14,
        T15,
        T16,
    > Service<Request<B>>
    for FromFn<
        F,
        S,
        (
            T1,
            T2,
            T3,
            T4,
            T5,
            T6,
            T7,
            T8,
            T9,
            T10,
            T11,
            T12,
            T13,
            T14,
            T15,
            T16,
        ),
    >
where
    F: FnMut(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, Next<B>) -> Fut
        + Clone
        + Send
        + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: IntoResponse + 'static,
    S: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Mut, B> + Send,
    T10: FromRequest<Mut, B> + Send,
    T11: FromRequest<Mut, B> + Send,
    T12: FromRequest<Mut, B> + Send,
    T13: FromRequest<Mut, B> + Send,
    T14: FromRequest<Mut, B> + Send,
    T15: FromRequest<Mut, B> + Send,
    T16: FromRequest<Once, B> + Send,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        let mut f = self.f.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T3 = match T3::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T4 = match T4::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T5 = match T5::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T6 = match T6::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T7 = match T7::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T8 = match T8::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T9 = match T9::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T10 = match T10::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T11 = match T11::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T12 = match T12::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T13 = match T13::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T14 = match T14::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T15 = match T15::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T16 = match T16::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let inner = ServiceBuilder::new()
                .boxed_clone()
                .map_response_body(body::boxed)
                .service(ready_inner);
            let next = Next { inner };
            f(
                T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, next,
            )
            .await
            .into_response()
        });
        ResponseFuture { inner: future }
    }
}
