//! Routing for handlers based on HTTP methods.

use crate::{
    body::{box_body, BoxBody},
    handler::Handler,
    routing::{MethodFilter, MethodNotAllowed},
    util::{Either, EitherProj},
};
use futures_util::{future::BoxFuture, ready};
use http::Method;
use http::{Request, Response};
use http_body::Empty;
use pin_project_lite::pin_project;
use std::{
    convert::Infallible,
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower::util::Oneshot;
use tower::ServiceExt;
use tower_service::Service;

/// Route requests with any standard HTTP method to the given handler.
///
/// # Example
///
/// ```rust
/// use axum::{
///     routing::any,
///     Router,
/// };
///
/// async fn handler() {}
///
/// let app = Router::new().route("/", any(handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Note that this only accepts the standard HTTP methods. If you need to
/// support non-standard methods use [`Handler::into_service`]:
///
/// ```rust
/// use axum::{
///     handler::Handler,
///     Router,
/// };
///
/// async fn handler() {}
///
/// let app = Router::new().route("/", handler.into_service());
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub fn any<H, B, T>(handler: H) -> MethodRouter<H, B, T, MethodNotAllowed>
where
    H: Handler<B, T>,
{
    on(MethodFilter::all(), handler)
}

/// Route `DELETE` requests to the given handler.
///
/// See [`get`] for an example.
pub fn delete<H, B, T>(handler: H) -> MethodRouter<H, B, T, MethodNotAllowed>
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
/// use axum::{
///     routing::get,
///     Router,
/// };
///
/// async fn handler() {}
///
/// // Requests to `GET /` will go to `handler`.
/// let app = Router::new().route("/", get(handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Note that `get` routes will also be called for `HEAD` requests but will have
/// the response body removed. Make sure to add explicit `HEAD` routes
/// afterwards.
pub fn get<H, B, T>(handler: H) -> MethodRouter<H, B, T, MethodNotAllowed>
where
    H: Handler<B, T>,
{
    on(MethodFilter::GET | MethodFilter::HEAD, handler)
}

/// Route `HEAD` requests to the given handler.
///
/// See [`get`] for an example.
pub fn head<H, B, T>(handler: H) -> MethodRouter<H, B, T, MethodNotAllowed>
where
    H: Handler<B, T>,
{
    on(MethodFilter::HEAD, handler)
}

/// Route `OPTIONS` requests to the given handler.
///
/// See [`get`] for an example.
pub fn options<H, B, T>(handler: H) -> MethodRouter<H, B, T, MethodNotAllowed>
where
    H: Handler<B, T>,
{
    on(MethodFilter::OPTIONS, handler)
}

/// Route `PATCH` requests to the given handler.
///
/// See [`get`] for an example.
pub fn patch<H, B, T>(handler: H) -> MethodRouter<H, B, T, MethodNotAllowed>
where
    H: Handler<B, T>,
{
    on(MethodFilter::PATCH, handler)
}

/// Route `POST` requests to the given handler.
///
/// See [`get`] for an example.
pub fn post<H, B, T>(handler: H) -> MethodRouter<H, B, T, MethodNotAllowed>
where
    H: Handler<B, T>,
{
    on(MethodFilter::POST, handler)
}

/// Route `PUT` requests to the given handler.
///
/// See [`get`] for an example.
pub fn put<H, B, T>(handler: H) -> MethodRouter<H, B, T, MethodNotAllowed>
where
    H: Handler<B, T>,
{
    on(MethodFilter::PUT, handler)
}

/// Route `TRACE` requests to the given handler.
///
/// See [`get`] for an example.
pub fn trace<H, B, T>(handler: H) -> MethodRouter<H, B, T, MethodNotAllowed>
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
/// use axum::{
///     routing::on,
///     Router,
///     routing::MethodFilter,
/// };
///
/// async fn handler() {}
///
/// // Requests to `POST /` will go to `handler`.
/// let app = Router::new().route("/", on(MethodFilter::POST, handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub fn on<H, B, T>(method: MethodFilter, handler: H) -> MethodRouter<H, B, T, MethodNotAllowed>
where
    H: Handler<B, T>,
{
    MethodRouter {
        method,
        handler,
        fallback: MethodNotAllowed::new(),
        _marker: PhantomData,
    }
}

/// A handler [`Service`] that accepts requests based on a [`MethodFilter`] and
/// allows chaining additional handlers.
pub struct MethodRouter<H, B, T, F> {
    pub(crate) method: MethodFilter,
    pub(crate) handler: H,
    pub(crate) fallback: F,
    pub(crate) _marker: PhantomData<fn() -> (B, T)>,
}

impl<H, B, T, F> fmt::Debug for MethodRouter<H, B, T, F>
where
    T: fmt::Debug,
    F: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MethodRouter")
            .field("method", &self.method)
            .field("handler", &format_args!("{}", std::any::type_name::<H>()))
            .field("fallback", &self.fallback)
            .finish()
    }
}

impl<H, B, T, F> Clone for MethodRouter<H, B, T, F>
where
    H: Clone,
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            method: self.method,
            handler: self.handler.clone(),
            fallback: self.fallback.clone(),
            _marker: PhantomData,
        }
    }
}

impl<H, B, T, F> Copy for MethodRouter<H, B, T, F>
where
    H: Copy,
    F: Copy,
{
}

impl<H, B, T, F> MethodRouter<H, B, T, F> {
    /// Chain an additional handler that will accept all requests regardless of
    /// its HTTP method.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn any<H2, T2>(self, handler: H2) -> MethodRouter<H2, B, T2, Self>
    where
        H2: Handler<B, T2>,
    {
        self.on(MethodFilter::all(), handler)
    }

    /// Chain an additional handler that will only accept `DELETE` requests.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn delete<H2, T2>(self, handler: H2) -> MethodRouter<H2, B, T2, Self>
    where
        H2: Handler<B, T2>,
    {
        self.on(MethodFilter::DELETE, handler)
    }

    /// Chain an additional handler that will only accept `GET` requests.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::{routing::post, Router};
    ///
    /// async fn handler() {}
    ///
    /// async fn other_handler() {}
    ///
    /// // Requests to `GET /` will go to `handler` and `POST /` will go to
    /// // `other_handler`.
    /// let app = Router::new().route("/", post(handler).get(other_handler));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// Note that `get` routes will also be called for `HEAD` requests but will have
    /// the response body removed. Make sure to add explicit `HEAD` routes
    /// afterwards.
    pub fn get<H2, T2>(self, handler: H2) -> MethodRouter<H2, B, T2, Self>
    where
        H2: Handler<B, T2>,
    {
        self.on(MethodFilter::GET | MethodFilter::HEAD, handler)
    }

    /// Chain an additional handler that will only accept `HEAD` requests.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn head<H2, T2>(self, handler: H2) -> MethodRouter<H2, B, T2, Self>
    where
        H2: Handler<B, T2>,
    {
        self.on(MethodFilter::HEAD, handler)
    }

    /// Chain an additional handler that will only accept `OPTIONS` requests.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn options<H2, T2>(self, handler: H2) -> MethodRouter<H2, B, T2, Self>
    where
        H2: Handler<B, T2>,
    {
        self.on(MethodFilter::OPTIONS, handler)
    }

    /// Chain an additional handler that will only accept `PATCH` requests.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn patch<H2, T2>(self, handler: H2) -> MethodRouter<H2, B, T2, Self>
    where
        H2: Handler<B, T2>,
    {
        self.on(MethodFilter::PATCH, handler)
    }

    /// Chain an additional handler that will only accept `POST` requests.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn post<H2, T2>(self, handler: H2) -> MethodRouter<H2, B, T2, Self>
    where
        H2: Handler<B, T2>,
    {
        self.on(MethodFilter::POST, handler)
    }

    /// Chain an additional handler that will only accept `PUT` requests.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn put<H2, T2>(self, handler: H2) -> MethodRouter<H2, B, T2, Self>
    where
        H2: Handler<B, T2>,
    {
        self.on(MethodFilter::PUT, handler)
    }

    /// Chain an additional handler that will only accept `TRACE` requests.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn trace<H2, T2>(self, handler: H2) -> MethodRouter<H2, B, T2, Self>
    where
        H2: Handler<B, T2>,
    {
        self.on(MethodFilter::TRACE, handler)
    }

    /// Chain an additional handler that will accept requests matching the given
    /// `MethodFilter`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::{
    ///     routing::get,
    ///     Router,
    ///     routing::MethodFilter
    /// };
    ///
    /// async fn handler() {}
    ///
    /// async fn other_handler() {}
    ///
    /// // Requests to `GET /` will go to `handler` and `DELETE /` will go to
    /// // `other_handler`
    /// let app = Router::new().route("/", get(handler).on(MethodFilter::DELETE, other_handler));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn on<H2, T2>(self, method: MethodFilter, handler: H2) -> MethodRouter<H2, B, T2, Self>
    where
        H2: Handler<B, T2>,
    {
        MethodRouter {
            method,
            handler,
            fallback: self,
            _marker: PhantomData,
        }
    }
}

impl<H, B, T, F> Service<Request<B>> for MethodRouter<H, B, T, F>
where
    H: Handler<B, T>,
    F: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible> + Clone,
    B: Send + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = MethodRouterFuture<F, B>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let req_method = req.method().clone();

        let fut = if self.method.matches(req.method()) {
            let fut = Handler::call(self.handler.clone(), req);
            Either::A { inner: fut }
        } else {
            let fut = self.fallback.clone().oneshot(req);
            Either::B { inner: fut }
        };

        MethodRouterFuture {
            inner: fut,
            req_method,
        }
    }
}

pin_project! {
    /// The response future for [`MethodRouter`].
    pub struct MethodRouterFuture<F, B>
    where
        F: Service<Request<B>>
    {
        #[pin]
        pub(super) inner: Either<
            BoxFuture<'static, Response<BoxBody>>,
            Oneshot<F, Request<B>>,
        >,
        pub(super) req_method: Method,
    }
}

impl<F, B> Future for MethodRouterFuture<F, B>
where
    F: Service<Request<B>, Response = Response<BoxBody>>,
{
    type Output = Result<Response<BoxBody>, F::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let response = match this.inner.project() {
            EitherProj::A { inner } => ready!(inner.poll(cx)),
            EitherProj::B { inner } => ready!(inner.poll(cx))?,
        };

        if this.req_method == &Method::HEAD {
            let response = response.map(|_| box_body(Empty::new()));
            Poll::Ready(Ok(response))
        } else {
            Poll::Ready(Ok(response))
        }
    }
}

impl<F, B> fmt::Debug for MethodRouterFuture<F, B>
where
    F: Service<Request<B>>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MethodRouterFuture").finish()
    }
}
