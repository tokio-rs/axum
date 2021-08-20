use super::Handler;
use crate::{
    body::{box_body, BoxBody},
    extract::{FromRequest, RequestParts},
    response::IntoResponse,
};
use http::{Request, Response};
use std::{
    convert::Infallible,
    fmt,
    marker::PhantomData,
    task::{Context, Poll},
};
use tower::Service;

/// An adapter that makes a [`Handler`] into a [`Service`].
///
/// Created with [`Handler::into_service`].
pub struct IntoService<H, T> {
    handler: H,
    _marker: PhantomData<fn() -> T>,
}

impl<H, T> IntoService<H, T> {
    pub(super) fn new(handler: H) -> Self {
        Self {
            handler,
            _marker: PhantomData,
        }
    }
}

impl<H, T> fmt::Debug for IntoService<H, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntoService")
            .field(&format_args!("..."))
            .finish()
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

impl<H, T, B> Service<Request<B>> for IntoService<H, T>
where
    H: Handler<T>,
    T: FromRequest<B> + Send,
    T::Rejection: Send,
    B: Send + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = super::future::IntoServiceFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // `IntoService` can only be constructed from async functions which are always ready, or from
        // `Layered` which bufferes in `<Layered as Handler>::call` and is therefore also always
        // ready.
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let handler = self.handler.clone();
        let future = Box::pin(async move {
            let mut req = RequestParts::new(req);
            let input = T::from_request(&mut req).await;
            let res = match input {
                Ok(input) => Handler::call(handler, input).await,
                Err(rejection) => rejection.into_response().map(box_body),
            };
            Ok::<_, Infallible>(res)
        });

        super::future::IntoServiceFuture { future }
    }
}
