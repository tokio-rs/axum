#![allow(unused_imports, dead_code)]

/*

Improvements to make:

Break stuff up into modules

Support extracting headers, perhaps via `headers::Header`?

Tests

*/

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{future, ready};
use http::{header, HeaderValue, Method, Request, Response, StatusCode};
use http_body::Body as _;
use pin_project::pin_project;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    convert::Infallible,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{BoxError, Layer, Service, ServiceExt};

mod body;
pub use body::BoxBody;

pub use hyper::body::Body;

pub fn app() -> App<EmptyRouter> {
    App {
        router: EmptyRouter(()),
    }
}

#[derive(Debug, Clone)]
pub struct App<R> {
    router: R,
}

impl<R> App<R> {
    pub fn at(self, route_spec: &str) -> RouteAt<R> {
        self.at_bytes(Bytes::copy_from_slice(route_spec.as_bytes()))
    }

    fn at_bytes(self, route_spec: Bytes) -> RouteAt<R> {
        RouteAt {
            app: self,
            route_spec,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RouteAt<R> {
    app: App<R>,
    route_spec: Bytes,
}

impl<R> RouteAt<R> {
    pub fn get<F, B, T>(self, handler_fn: F) -> RouteBuilder<Route<HandlerSvc<F, B, T>, R>>
    where
        F: Handler<B, T>,
    {
        self.add_route(handler_fn, Method::GET)
    }

    pub fn get_service<S, B>(self, service: S) -> RouteBuilder<Route<S, R>>
    where
        S: Service<Request<Body>, Response = Response<B>> + Clone,
        S::Error: Into<BoxError>,
    {
        self.add_route_service(service, Method::GET)
    }

    pub fn post<F, B, T>(self, handler_fn: F) -> RouteBuilder<Route<HandlerSvc<F, B, T>, R>>
    where
        F: Handler<B, T>,
    {
        self.add_route(handler_fn, Method::POST)
    }

    pub fn post_service<S, B>(self, service: S) -> RouteBuilder<Route<S, R>>
    where
        S: Service<Request<Body>, Response = Response<B>> + Clone,
        S::Error: Into<BoxError>,
    {
        self.add_route_service(service, Method::POST)
    }

    fn add_route<H, B, T>(
        self,
        handler: H,
        method: Method,
    ) -> RouteBuilder<Route<HandlerSvc<H, B, T>, R>>
    where
        H: Handler<B, T>,
    {
        self.add_route_service(HandlerSvc::new(handler), method)
    }

    fn add_route_service<S>(self, service: S, method: Method) -> RouteBuilder<Route<S, R>> {
        let new_app = App {
            router: Route {
                service,
                route_spec: RouteSpec {
                    method,
                    spec: self.route_spec.clone(),
                },
                fallback: self.app.router,
                handler_ready: false,
                fallback_ready: false,
            },
        };

        RouteBuilder {
            app: new_app,
            route_spec: self.route_spec,
        }
    }
}

pub struct RouteBuilder<R> {
    app: App<R>,
    route_spec: Bytes,
}

impl<R> Clone for RouteBuilder<R>
where
    R: Clone,
{
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            route_spec: self.route_spec.clone(),
        }
    }
}

impl<R> RouteBuilder<R> {
    pub fn at(self, route_spec: &str) -> RouteAt<R> {
        self.app.at(route_spec)
    }

    pub fn get<F, B, T>(self, handler_fn: F) -> RouteBuilder<Route<HandlerSvc<F, B, T>, R>>
    where
        F: Handler<B, T>,
    {
        self.app.at_bytes(self.route_spec).get(handler_fn)
    }

    pub fn get_service<S, B>(self, service: S) -> RouteBuilder<Route<S, R>>
    where
        S: Service<Request<Body>, Response = Response<B>> + Clone,
        S::Error: Into<BoxError>,
    {
        self.app.at_bytes(self.route_spec).get_service(service)
    }

    pub fn post<F, B, T>(self, handler_fn: F) -> RouteBuilder<Route<HandlerSvc<F, B, T>, R>>
    where
        F: Handler<B, T>,
    {
        self.app.at_bytes(self.route_spec).post(handler_fn)
    }

    pub fn post_service<S, B>(self, service: S) -> RouteBuilder<Route<S, R>>
    where
        S: Service<Request<Body>, Response = Response<B>> + Clone,
        S::Error: Into<BoxError>,
    {
        self.app.at_bytes(self.route_spec).post_service(service)
    }

    pub fn into_service(self) -> IntoService<R> {
        IntoService {
            app: self.app,
            poll_ready_error: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("failed to deserialize the request body")]
    DeserializeRequestBody(#[source] serde_json::Error),

    #[error("failed to serialize the response body")]
    SerializeResponseBody(#[source] serde_json::Error),

    #[error("failed to consume the body")]
    ConsumeRequestBody(#[source] hyper::Error),

    #[error("URI contained no query string")]
    QueryStringMissing,

    #[error("failed to deserialize query string")]
    DeserializeQueryString(#[source] serde_urlencoded::de::Error),

    #[error("failed generating the response body")]
    ResponseBody(#[source] BoxError),

    #[error("handler service returned an error")]
    Service(#[source] BoxError),

    #[error("request extension was not set")]
    MissingExtension { type_name: &'static str },
}

impl From<Infallible> for Error {
    fn from(err: Infallible) -> Self {
        match err {}
    }
}

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

    async fn call(self, req: Request<Body>) -> Result<Self::Response, Error>;

    fn layer<L>(self, layer: L) -> Layered<L::Service, In>
    where
        L: Layer<HandlerSvc<Self, B, In>>,
    {
        Layered::new(layer.layer(HandlerSvc::new(self)))
    }
}

pub trait IntoResponse<B> {
    fn into_response(self) -> Result<Response<B>, Error>;
}

impl<B> IntoResponse<B> for Response<B> {
    fn into_response(self) -> Result<Response<B>, Error> {
        Ok(self)
    }
}

impl IntoResponse<Body> for &'static str {
    fn into_response(self) -> Result<Response<Body>, Error> {
        Ok(Response::new(Body::from(self)))
    }
}

impl IntoResponse<Body> for String {
    fn into_response(self) -> Result<Response<Body>, Error> {
        Ok(Response::new(Body::from(self)))
    }
}

impl IntoResponse<Body> for Bytes {
    fn into_response(self) -> Result<Response<Body>, Error> {
        Ok(Response::new(Body::from(self)))
    }
}

impl IntoResponse<Body> for &'static [u8] {
    fn into_response(self) -> Result<Response<Body>, Error> {
        Ok(Response::new(Body::from(self)))
    }
}

impl IntoResponse<Body> for Vec<u8> {
    fn into_response(self) -> Result<Response<Body>, Error> {
        Ok(Response::new(Body::from(self)))
    }
}

impl IntoResponse<Body> for std::borrow::Cow<'static, str> {
    fn into_response(self) -> Result<Response<Body>, Error> {
        Ok(Response::new(Body::from(self)))
    }
}

impl IntoResponse<Body> for std::borrow::Cow<'static, [u8]> {
    fn into_response(self) -> Result<Response<Body>, Error> {
        Ok(Response::new(Body::from(self)))
    }
}

// TODO(david): rename this to Json when its in another module
pub struct JsonBody<T>(T);

impl<T> IntoResponse<Body> for JsonBody<T>
where
    T: Serialize,
{
    fn into_response(self) -> Result<Response<Body>, Error> {
        let bytes = serde_json::to_vec(&self.0).map_err(Error::SerializeResponseBody)?;
        let len = bytes.len();
        let mut res = Response::new(Body::from(bytes));

        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        res.headers_mut()
            .insert(header::CONTENT_LENGTH, HeaderValue::from(len));

        Ok(res)
    }
}

#[async_trait]
impl<F, Fut, B, Res> Handler<B, ()> for F
where
    F: Fn(Request<Body>) -> Fut + Send + Sync,
    Fut: Future<Output = Result<Res, Error>> + Send,
    Res: IntoResponse<B>,
{
    type Response = Res;

    type Sealed = sealed::Hidden;

    async fn call(self, req: Request<Body>) -> Result<Self::Response, Error> {
        self(req).await
    }
}

macro_rules! impl_handler {
    ( $head:ident $(,)? ) => {
        #[async_trait]
        #[allow(non_snake_case)]
        impl<F, Fut, B, Res, $head> Handler<B, ($head,)> for F
        where
            F: Fn(Request<Body>, $head) -> Fut + Send + Sync,
            Fut: Future<Output = Result<Res, Error>> + Send,
            Res: IntoResponse<B>,
            $head: FromRequest + Send,
        {
            type Response = Res;

            type Sealed = sealed::Hidden;

            async fn call(self, mut req: Request<Body>) -> Result<Self::Response, Error> {
                let $head = $head::from_request(&mut req).await?;
                let res = self(req, $head).await?;
                Ok(res)
            }
        }
    };

    ( $head:ident, $($tail:ident),* $(,)? ) => {
        #[async_trait]
        #[allow(non_snake_case)]
        impl<F, Fut, B, Res, $head, $($tail,)*> Handler<B, ($head, $($tail,)*)> for F
        where
            F: Fn(Request<Body>, $head, $($tail,)*) -> Fut + Send + Sync,
            Fut: Future<Output = Result<Res, Error>> + Send,
            Res: IntoResponse<B>,
            $head: FromRequest + Send,
            $( $tail: FromRequest + Send, )*
        {
            type Response = Res;

            type Sealed = sealed::Hidden;

            async fn call(self, mut req: Request<Body>) -> Result<Self::Response, Error> {
                let $head = $head::from_request(&mut req).await?;
                $(
                    let $tail = $tail::from_request(&mut req).await?;
                )*
                let res = self(req, $head, $($tail,)*).await?;
                Ok(res)
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
    fn new(svc: S) -> Self {
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
    fn new(handler: H) -> Self {
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

pub trait FromRequest: Sized {
    type Future: Future<Output = Result<Self, Error>> + Send;

    fn from_request(req: &mut Request<Body>) -> Self::Future;
}

impl<T> FromRequest for Option<T>
where
    T: FromRequest,
{
    type Future = OptionFromRequestFuture<T::Future>;

    fn from_request(req: &mut Request<Body>) -> Self::Future {
        OptionFromRequestFuture(T::from_request(req))
    }
}

#[pin_project]
pub struct OptionFromRequestFuture<F>(#[pin] F);

impl<F, T> Future for OptionFromRequestFuture<F>
where
    F: Future<Output = Result<T, Error>>,
{
    type Output = Result<Option<T>, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let value = ready!(self.project().0.poll(cx));
        Poll::Ready(Ok(value.ok()))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Query<T>(T);

impl<T> Query<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> FromRequest for Query<T>
where
    T: DeserializeOwned + Send,
{
    type Future = future::Ready<Result<Self, Error>>;

    fn from_request(req: &mut Request<Body>) -> Self::Future {
        let result = (|| {
            let query = req.uri().query().ok_or(Error::QueryStringMissing)?;
            let value = serde_urlencoded::from_str(query).map_err(Error::DeserializeQueryString)?;
            Ok(Query(value))
        })();

        future::ready(result)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Json<T>(T);

impl<T> Json<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> FromRequest for Json<T>
where
    T: DeserializeOwned,
{
    type Future = future::BoxFuture<'static, Result<Self, Error>>;

    fn from_request(req: &mut Request<Body>) -> Self::Future {
        // TODO(david): require the body to have `content-type: application/json`

        let body = std::mem::take(req.body_mut());

        Box::pin(async move {
            let bytes = hyper::body::to_bytes(body)
                .await
                .map_err(Error::ConsumeRequestBody)?;
            let value = serde_json::from_slice(&bytes).map_err(Error::DeserializeRequestBody)?;
            Ok(Json(value))
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Extension<T>(T);

impl<T> Extension<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> FromRequest for Extension<T>
where
    T: Clone + Send + Sync + 'static,
{
    type Future = future::Ready<Result<Self, Error>>;

    fn from_request(req: &mut Request<Body>) -> Self::Future {
        let result = (|| {
            let value = req
                .extensions()
                .get::<T>()
                .ok_or_else(|| Error::MissingExtension {
                    type_name: std::any::type_name::<T>(),
                })
                .map(|x| x.clone())?;
            Ok(Extension(value))
        })();

        future::ready(result)
    }
}

// TODO(david): rename this to Bytes when its in another module
#[derive(Debug, Clone)]
pub struct BytesBody(Bytes);

impl BytesBody {
    pub fn into_inner(self) -> Bytes {
        self.0
    }
}

impl FromRequest for BytesBody {
    type Future = future::BoxFuture<'static, Result<Self, Error>>;

    fn from_request(req: &mut Request<Body>) -> Self::Future {
        let body = std::mem::take(req.body_mut());

        Box::pin(async move {
            let bytes = hyper::body::to_bytes(body)
                .await
                .map_err(Error::ConsumeRequestBody)?;
            Ok(BytesBody(bytes))
        })
    }
}

#[derive(Clone, Copy)]
pub struct EmptyRouter(());

impl<R> Service<R> for EmptyRouter {
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: R) -> Self::Future {
        let mut res = Response::new(Body::empty());
        *res.status_mut() = StatusCode::NOT_FOUND;
        future::ok(res)
    }
}

pub struct Route<H, F> {
    service: H,
    route_spec: RouteSpec,
    fallback: F,
    handler_ready: bool,
    fallback_ready: bool,
}

impl<H, F> Clone for Route<H, F>
where
    H: Clone,
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            service: self.service.clone(),
            fallback: self.fallback.clone(),
            route_spec: self.route_spec.clone(),
            // important to reset readiness when cloning
            handler_ready: false,
            fallback_ready: false,
        }
    }
}

#[derive(Clone)]
struct RouteSpec {
    method: Method,
    spec: Bytes,
}

impl RouteSpec {
    fn matches<B>(&self, req: &Request<B>) -> bool {
        // TODO(david): support dynamic placeholders like `/users/:id`
        req.method() == self.method && req.uri().path().as_bytes() == self.spec
    }
}

impl<H, F, HB, FB> Service<Request<Body>> for Route<H, F>
where
    H: Service<Request<Body>, Response = Response<HB>>,
    H::Error: Into<Error>,
    HB: http_body::Body + Send + Sync + 'static,
    HB::Error: Into<BoxError>,

    F: Service<Request<Body>, Response = Response<FB>>,
    F::Error: Into<Error>,
    FB: http_body::Body<Data = HB::Data> + Send + Sync + 'static,
    FB::Error: Into<BoxError>,
{
    type Response = Response<BoxBody<HB::Data, Error>>;
    type Error = Error;
    type Future = future::Either<BoxResponseBody<H::Future>, BoxResponseBody<F::Future>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        loop {
            if !self.handler_ready {
                ready!(self.service.poll_ready(cx)).map_err(Into::into)?;
                self.handler_ready = true;
            }

            if !self.fallback_ready {
                ready!(self.fallback.poll_ready(cx)).map_err(Into::into)?;
                self.fallback_ready = true;
            }

            if self.handler_ready && self.fallback_ready {
                return Poll::Ready(Ok(()));
            }
        }
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        if self.route_spec.matches(&req) {
            assert!(
                self.handler_ready,
                "handler not ready. Did you forget to call `poll_ready`?"
            );
            self.handler_ready = false;
            future::Either::Left(BoxResponseBody(self.service.call(req)))
        } else {
            assert!(
                self.fallback_ready,
                "fallback not ready. Did you forget to call `poll_ready`?"
            );
            self.fallback_ready = false;
            // TODO(david): this leads to each route creating one box body, probably not great
            future::Either::Right(BoxResponseBody(self.fallback.call(req)))
        }
    }
}

#[pin_project]
pub struct BoxResponseBody<F>(#[pin] F);

impl<F, B, E> Future for BoxResponseBody<F>
where
    F: Future<Output = Result<Response<B>, E>>,
    E: Into<Error>,
    B: http_body::Body + Send + Sync + 'static,
    B::Error: Into<BoxError>,
{
    type Output = Result<Response<BoxBody<B::Data, Error>>, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let response: Response<B> = ready!(self.project().0.poll(cx)).map_err(Into::into)?;
        let response = response.map(|body| {
            let body = body.map_err(|err| Error::ResponseBody(err.into()));
            BoxBody::new(body)
        });
        Poll::Ready(Ok(response))
    }
}

pub struct IntoService<R> {
    app: App<R>,
    poll_ready_error: Option<Error>,
}

impl<R> Clone for IntoService<R>
where
    R: Clone,
{
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            poll_ready_error: None,
        }
    }
}

impl<R, B, T> Service<T> for IntoService<R>
where
    R: Service<T, Response = Response<B>>,
    R::Error: Into<Error>,
    B: Default,
{
    type Response = Response<B>;
    type Error = Error;
    type Future = HandleErrorFuture<R::Future, B>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.app.router.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: T) -> Self::Future {
        if let Some(poll_ready_error) = self.poll_ready_error.take() {
            match handle_error::<B>(poll_ready_error) {
                Ok(res) => {
                    return HandleErrorFuture(Kind::Response(Some(res)));
                }
                Err(err) => {
                    return HandleErrorFuture(Kind::Error(Some(err)));
                }
            }
        }
        HandleErrorFuture(Kind::Future(self.app.router.call(req)))
    }
}

#[pin_project]
pub struct HandleErrorFuture<F, B>(#[pin] Kind<F, B>);

#[pin_project(project = KindProj)]
enum Kind<F, B> {
    Response(Option<Response<B>>),
    Error(Option<Error>),
    Future(#[pin] F),
}

impl<F, B, E> Future for HandleErrorFuture<F, B>
where
    F: Future<Output = Result<Response<B>, E>>,
    E: Into<Error>,
    B: Default,
{
    type Output = Result<Response<B>, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().0.project() {
            KindProj::Response(res) => Poll::Ready(Ok(res.take().unwrap())),
            KindProj::Error(err) => Poll::Ready(Err(err.take().unwrap())),
            KindProj::Future(fut) => match ready!(fut.poll(cx)) {
                Ok(res) => Poll::Ready(Ok(res)),
                Err(err) => Poll::Ready(handle_error(err.into())),
            },
        }
    }
}

fn handle_error<B>(error: Error) -> Result<Response<B>, Error>
where
    B: Default,
{
    fn make_response<B>(status: StatusCode) -> Result<Response<B>, Error>
    where
        B: Default,
    {
        let mut res = Response::new(B::default());
        *res.status_mut() = status;
        Ok(res)
    }

    match error {
        Error::DeserializeRequestBody(_)
        | Error::QueryStringMissing
        | Error::DeserializeQueryString(_) => make_response(StatusCode::BAD_REQUEST),

        Error::MissingExtension { .. } | Error::SerializeResponseBody(_) => {
            make_response(StatusCode::INTERNAL_SERVER_ERROR)
        }

        Error::Service(err) => match err.downcast::<Error>() {
            Ok(err) => Err(*err),
            Err(err) => Err(Error::Service(err)),
        },

        err @ Error::ConsumeRequestBody(_) => Err(err),
        err @ Error::ResponseBody(_) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    #![allow(warnings)]
    use super::*;
    use hyper::Server;
    use std::time::Duration;
    use std::{fmt, net::SocketAddr, sync::Arc};
    use tower::{
        layer::util::Identity, make::Shared, service_fn, timeout::TimeoutLayer, ServiceBuilder,
    };
    use tower_http::{
        add_extension::AddExtensionLayer,
        compression::CompressionLayer,
        trace::{Trace, TraceLayer},
    };

    #[tokio::test]
    async fn basic() {
        #[derive(Debug, Deserialize)]
        struct Pagination {
            page: usize,
            per_page: usize,
        }

        #[derive(Debug, Deserialize)]
        struct UsersCreate {
            username: String,
        }

        async fn root(_: Request<Body>) -> Result<Response<Body>, Error> {
            Ok(Response::new(Body::from("Hello, World!")))
        }

        async fn large_static_file(_: Request<Body>) -> Result<Response<Body>, Error> {
            Ok(Response::new(Body::empty()))
        }

        let app =
            app()
                // routes with functions
                .at("/")
                .get(root)
                // routes with closures
                .at("/users")
                .get(|_: Request<Body>, pagination: Query<Pagination>| async {
                    let pagination = pagination.into_inner();
                    assert_eq!(pagination.page, 1);
                    assert_eq!(pagination.per_page, 30);
                    Ok::<_, Error>("users#index".to_string())
                })
                .post(
                    |_: Request<Body>,
                     payload: Json<UsersCreate>,
                     _state: Extension<Arc<State>>| async {
                        let payload = payload.into_inner();
                        assert_eq!(payload.username, "bob");
                        Ok::<_, Error>(JsonBody(
                            serde_json::json!({ "username": payload.username }),
                        ))
                    },
                )
                // routes with a service
                .at("/service")
                .get_service(service_fn(root))
                // routes with layers applied
                .at("/large-static-file")
                .get(
                    large_static_file.layer(
                        ServiceBuilder::new()
                            .layer(TimeoutLayer::new(Duration::from_secs(30)))
                            .layer(CompressionLayer::new())
                            .into_inner(),
                    ),
                )
                .into_service();

        // state shared by all routes, could hold db connection etc
        struct State {}

        let state = Arc::new(State {});

        // can add more middleware
        let mut app = ServiceBuilder::new()
            .layer(AddExtensionLayer::new(state))
            .layer(TraceLayer::new_for_http())
            .service(app);

        let res = app
            .ready()
            .await
            .unwrap()
            .call(
                Request::builder()
                    .method(Method::GET)
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_to_string(res).await, "Hello, World!");

        let res = app
            .ready()
            .await
            .unwrap()
            .call(
                Request::builder()
                    .method(Method::GET)
                    .uri("/users?page=1&per_page=30")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_to_string(res).await, "users#index");

        let res = app
            .ready()
            .await
            .unwrap()
            .call(
                Request::builder()
                    .method(Method::GET)
                    .uri("/users")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body_to_string(res).await, "");

        let res = app
            .ready()
            .await
            .unwrap()
            .call(
                Request::builder()
                    .method(Method::POST)
                    .uri("/users")
                    .body(Body::from(r#"{ "username": "bob" }"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_to_string(res).await, r#"{"username":"bob"}"#);
    }

    async fn body_to_string<B>(res: Response<B>) -> String
    where
        B: http_body::Body,
        B::Error: fmt::Debug,
    {
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[allow(dead_code)]
    // this should just compile
    async fn compatible_with_hyper_and_tower_http() {
        let app = app()
            .at("/")
            .get(|_: Request<Body>| async {
                Ok::<_, Error>(Response::new(Body::from("Hello, World!")))
            })
            .into_service();

        let app = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(CompressionLayer::new())
            .service(app);

        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
        let server = Server::bind(&addr).serve(Shared::new(app));
        server.await.unwrap();
    }
}
