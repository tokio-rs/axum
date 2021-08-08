//! Convert an extractor into a middleware.
//!
//! See [`extractor_middleware`] for more details.

use super::{FromRequest, RequestParts};
use crate::{body::BoxBody, response::IntoResponse};
use bytes::Bytes;
use futures_util::{future::BoxFuture, ready};
use http::{Request, Response};
use pin_project_lite::pin_project;
use std::{
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{BoxError, Layer, Service};

/// Convert an extractor into a middleware.
///
/// If the extractor succeeds the value will be discarded and the inner service
/// will be called. If the extractor fails the rejection will be returned and
/// the inner service will _not_ be called.
///
/// This can be used to perform validation of requests if the validation doesn't
/// produce any useful output, and run the extractor for several handlers
/// without repeating it in the function signature.
///
/// Note that if the extractor consumes the request body, as `String` or
/// [`Bytes`] does, an empty body will be left in its place. Thus wont be
/// accessible to subsequent extractors or handlers.
///
/// # Example
///
/// ```rust
/// use axum::{extract::{extractor_middleware, RequestParts}, prelude::*};
/// use http::StatusCode;
/// use async_trait::async_trait;
///
/// // An extractor that performs authorization.
/// struct RequireAuth;
///
/// #[async_trait]
/// impl<B> extract::FromRequest<B> for RequireAuth
/// where
///     B: Send,
/// {
///     type Rejection = StatusCode;
///
///     async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
///         let auth_header = req
///             .headers()
///             .and_then(|headers| headers.get(http::header::AUTHORIZATION))
///             .and_then(|value| value.to_str().ok());
///
///         if let Some(value) = auth_header {
///             if value == "secret" {
///                 return Ok(Self);
///             }
///         }
///
///         Err(StatusCode::UNAUTHORIZED)
///     }
/// }
///
/// async fn handler() {
///     // If we get here the request has been authorized
/// }
///
/// async fn other_handler() {
///     // If we get here the request has been authorized
/// }
///
/// let app = route("/", get(handler))
///     .route("/foo", post(other_handler))
///     // The extractor will run before all routes
///     .layer(extractor_middleware::<RequireAuth>());
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub fn extractor_middleware<E>() -> ExtractorMiddlewareLayer<E> {
    ExtractorMiddlewareLayer(PhantomData)
}

/// [`Layer`] that applies [`ExtractorMiddleware`] that runs an extractor and
/// discards the value.
///
/// See [`extractor_middleware`] for more details.
///
/// [`Layer`]: tower::Layer
pub struct ExtractorMiddlewareLayer<E>(PhantomData<fn() -> E>);

impl<E> Clone for ExtractorMiddlewareLayer<E> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl<E> fmt::Debug for ExtractorMiddlewareLayer<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtractorMiddleware")
            .field("extractor", &format_args!("{}", std::any::type_name::<E>()))
            .finish()
    }
}

impl<E, S> Layer<S> for ExtractorMiddlewareLayer<E> {
    type Service = ExtractorMiddleware<S, E>;

    fn layer(&self, inner: S) -> Self::Service {
        ExtractorMiddleware {
            inner,
            _extractor: PhantomData,
        }
    }
}

/// Middleware that runs an extractor and discards the value.
///
/// See [`extractor_middleware`] for more details.
pub struct ExtractorMiddleware<S, E> {
    inner: S,
    _extractor: PhantomData<fn() -> E>,
}

impl<S, E> Clone for ExtractorMiddleware<S, E>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _extractor: PhantomData,
        }
    }
}

impl<S, E> fmt::Debug for ExtractorMiddleware<S, E>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtractorMiddleware")
            .field("inner", &self.inner)
            .field("extractor", &format_args!("{}", std::any::type_name::<E>()))
            .finish()
    }
}

impl<S, E, ReqBody, ResBody> Service<Request<ReqBody>> for ExtractorMiddleware<S, E>
where
    E: FromRequest<ReqBody> + 'static,
    ReqBody: Default + Send + 'static,
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone,
    ResBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    ResBody::Error: Into<BoxError>,
{
    type Response = Response<BoxBody>;
    type Error = S::Error;
    type Future = ResponseFuture<ReqBody, S, E>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let extract_future = Box::pin(async move {
            let mut req = super::RequestParts::new(req);
            let extracted = E::from_request(&mut req).await;
            (req, extracted)
        });

        ResponseFuture {
            state: State::Extracting {
                future: extract_future,
            },
            svc: Some(self.inner.clone()),
        }
    }
}

pin_project! {
    /// Response future for [`ExtractorMiddleware`].
    #[allow(missing_debug_implementations)]
    pub struct ResponseFuture<ReqBody, S, E>
    where
        E: FromRequest<ReqBody>,
        S: Service<Request<ReqBody>>,
    {
        #[pin]
        state: State<ReqBody, S, E>,
        svc: Option<S>,
    }
}

pin_project! {
    #[project = StateProj]
    enum State<ReqBody, S, E>
    where
        E: FromRequest<ReqBody>,
        S: Service<Request<ReqBody>>,
    {
        Extracting { future: BoxFuture<'static, (RequestParts<ReqBody>, Result<E, E::Rejection>)> },
        Call { #[pin] future: S::Future },
    }
}

impl<ReqBody, S, E, ResBody> Future for ResponseFuture<ReqBody, S, E>
where
    E: FromRequest<ReqBody>,
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    ReqBody: Default,
    ResBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    ResBody::Error: Into<BoxError>,
{
    type Output = Result<Response<BoxBody>, S::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();

            let new_state = match this.state.as_mut().project() {
                StateProj::Extracting { future } => {
                    let (req, extracted) = ready!(future.as_mut().poll(cx));

                    match extracted {
                        Ok(_) => {
                            let mut svc = this.svc.take().expect("future polled after completion");
                            let req = req.try_into_request().unwrap_or_default();
                            let future = svc.call(req);
                            State::Call { future }
                        }
                        Err(err) => {
                            let res = err.into_response().map(crate::body::box_body);
                            return Poll::Ready(Ok(res));
                        }
                    }
                }
                StateProj::Call { future } => {
                    return future
                        .poll(cx)
                        .map(|result| result.map(|response| response.map(crate::body::box_body)));
                }
            };

            this.state.set(new_state);
        }
    }
}
