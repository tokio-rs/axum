// this file is machine generated. Don't edit it!

use super::*;

impl<F, Fut, Res, B, T1> Handler<(T1,), B> for F
where
    F: FnOnce(T1) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
    T1: FromRequest<Once, B> + Send,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
            let mut req = RequestParts::<Mut, B>::new(req);
            let mut req = RequestParts::<Once, B>::new(req.into_request());
            let T1 = match T1::from_request(&mut req).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let res = self(T1).await;
            res.into_response()
        })
    }
}
impl<F, Fut, Res, B, T1, T2> Handler<(T1, T2), B> for F
where
    F: FnOnce(T1, T2) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Once, B> + Send,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
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
            let res = self(T1, T2).await;
            res.into_response()
        })
    }
}
impl<F, Fut, Res, B, T1, T2, T3> Handler<(T1, T2, T3), B> for F
where
    F: FnOnce(T1, T2, T3) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Once, B> + Send,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
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
            let res = self(T1, T2, T3).await;
            res.into_response()
        })
    }
}
impl<F, Fut, Res, B, T1, T2, T3, T4> Handler<(T1, T2, T3, T4), B> for F
where
    F: FnOnce(T1, T2, T3, T4) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Once, B> + Send,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
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
            let res = self(T1, T2, T3, T4).await;
            res.into_response()
        })
    }
}
impl<F, Fut, Res, B, T1, T2, T3, T4, T5> Handler<(T1, T2, T3, T4, T5), B> for F
where
    F: FnOnce(T1, T2, T3, T4, T5) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Once, B> + Send,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
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
            let res = self(T1, T2, T3, T4, T5).await;
            res.into_response()
        })
    }
}
impl<F, Fut, Res, B, T1, T2, T3, T4, T5, T6> Handler<(T1, T2, T3, T4, T5, T6), B> for F
where
    F: FnOnce(T1, T2, T3, T4, T5, T6) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Once, B> + Send,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
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
            let res = self(T1, T2, T3, T4, T5, T6).await;
            res.into_response()
        })
    }
}
impl<F, Fut, Res, B, T1, T2, T3, T4, T5, T6, T7> Handler<(T1, T2, T3, T4, T5, T6, T7), B> for F
where
    F: FnOnce(T1, T2, T3, T4, T5, T6, T7) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Once, B> + Send,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
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
            let res = self(T1, T2, T3, T4, T5, T6, T7).await;
            res.into_response()
        })
    }
}
impl<F, Fut, Res, B, T1, T2, T3, T4, T5, T6, T7, T8> Handler<(T1, T2, T3, T4, T5, T6, T7, T8), B>
    for F
where
    F: FnOnce(T1, T2, T3, T4, T5, T6, T7, T8) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Once, B> + Send,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
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
            let res = self(T1, T2, T3, T4, T5, T6, T7, T8).await;
            res.into_response()
        })
    }
}
impl<F, Fut, Res, B, T1, T2, T3, T4, T5, T6, T7, T8, T9>
    Handler<(T1, T2, T3, T4, T5, T6, T7, T8, T9), B> for F
where
    F: FnOnce(T1, T2, T3, T4, T5, T6, T7, T8, T9) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
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
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
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
            let res = self(T1, T2, T3, T4, T5, T6, T7, T8, T9).await;
            res.into_response()
        })
    }
}
impl<F, Fut, Res, B, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10>
    Handler<(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10), B> for F
where
    F: FnOnce(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
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
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
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
            let res = self(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10).await;
            res.into_response()
        })
    }
}
impl<F, Fut, Res, B, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11>
    Handler<(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11), B> for F
where
    F: FnOnce(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
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
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
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
            let res = self(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11).await;
            res.into_response()
        })
    }
}
impl<F, Fut, Res, B, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12>
    Handler<(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12), B> for F
where
    F: FnOnce(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
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
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
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
            let res = self(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12).await;
            res.into_response()
        })
    }
}
impl<F, Fut, Res, B, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13>
    Handler<(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13), B> for F
where
    F: FnOnce(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13) -> Fut
        + Clone
        + Send
        + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
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
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
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
            let res = self(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13).await;
            res.into_response()
        })
    }
}
impl<F, Fut, Res, B, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14>
    Handler<(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14), B> for F
where
    F: FnOnce(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14) -> Fut
        + Clone
        + Send
        + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
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
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
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
            let res = self(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14).await;
            res.into_response()
        })
    }
}
impl<F, Fut, Res, B, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15>
    Handler<
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
        B,
    > for F
where
    F: FnOnce(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15) -> Fut
        + Clone
        + Send
        + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
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
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
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
            let res = self(
                T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15,
            )
            .await;
            res.into_response()
        })
    }
}
impl<F, Fut, Res, B, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16>
    Handler<
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
        B,
    > for F
where
    F: FnOnce(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16) -> Fut
        + Clone
        + Send
        + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
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
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    #[allow(non_snake_case, unused_mut)]
    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
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
            let res = self(
                T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16,
            )
            .await;
            res.into_response()
        })
    }
}
