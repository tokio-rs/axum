#![allow(unused_imports, dead_code)]

/*

Improvements to make:

Somehow return generic "into response" kinda types without having to manually
create hyper::Body for everything

Don't make Query and Json contain a Result, instead make generic wrapper
for "optional" inputs

Make it possible to convert QueryError and JsonError into responses

Support wrapping single routes in tower::Layer

Support putting a tower::Service at a Route

Don't require the response body to be hyper::Body, wont work if we're wrapping
single routes in layers

Support extracting headers, perhaps via `headers::Header`?

Implement `FromRequest` for more functions, with macro

Tests

*/

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{future, ready};
use http::{Method, Request, Response, StatusCode};
use http_body::{combinators::BoxBody, Body as _};
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
    pub fn get<F, T>(self, handler_fn: F) -> RouteBuilder<Route<HandlerSvc<F, T>, R>>
    where
        F: Handler<T>,
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

    pub fn post<F, T>(self, handler_fn: F) -> RouteBuilder<Route<HandlerSvc<F, T>, R>>
    where
        F: Handler<T>,
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

    fn add_route<H, T>(self, handler: H, method: Method) -> RouteBuilder<Route<HandlerSvc<H, T>, R>>
    where
        H: Handler<T>,
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

#[derive(Clone)]
pub struct RouteBuilder<R> {
    app: App<R>,
    route_spec: Bytes,
}

impl<R> RouteBuilder<R> {
    pub fn at(self, route_spec: &str) -> RouteAt<R> {
        self.app.at(route_spec)
    }

    pub fn get<F, T>(self, handler_fn: F) -> RouteBuilder<Route<HandlerSvc<F, T>, R>>
    where
        F: Handler<T>,
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

    pub fn post<F, T>(self, handler_fn: F) -> RouteBuilder<Route<HandlerSvc<F, T>, R>>
    where
        F: Handler<T>,
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
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("failed to deserialize the request body")]
    DeserializeRequestBody(#[source] serde_json::Error),

    #[error("failed to consume the body")]
    ConsumeBody(#[source] hyper::Error),

    #[error("URI contained no query string")]
    QueryStringMissing,

    #[error("failed to deserialize query string")]
    DeserializeQueryString(#[from] serde_urlencoded::de::Error),

    #[error("failed generating the response body")]
    ResponseBody(#[source] BoxError),

    #[error("handler service returned an error")]
    Service(#[source] BoxError),
}

impl From<BoxError> for Error {
    fn from(err: BoxError) -> Self {
        match err.downcast::<Error>() {
            Ok(err) => *err,
            Err(err) => Error::Service(err),
        }
    }
}

impl From<Infallible> for Error {
    fn from(err: Infallible) -> Self {
        match err {}
    }
}

// TODO(david): make this trait sealed
#[async_trait]
pub trait Handler<In>: Sized {
    type ResponseBody;

    async fn call(self, req: Request<Body>) -> Result<Response<Self::ResponseBody>, Error>;

    fn layer<L>(self, layer: L) -> Layered<L::Service, In>
    where
        L: Layer<HandlerSvc<Self, In>>,
    {
        Layered::new(layer.layer(HandlerSvc::new(self)))
    }
}

#[async_trait]
impl<F, Fut, B> Handler<()> for F
where
    F: Fn(Request<Body>) -> Fut + Send + Sync,
    Fut: Future<Output = Result<Response<B>, Error>> + Send,
{
    type ResponseBody = B;

    async fn call(self, req: Request<Body>) -> Result<Response<Self::ResponseBody>, Error> {
        self(req).await
    }
}

macro_rules! impl_handler {
    ( $head:ident $(,)? ) => {
        #[async_trait]
        #[allow(non_snake_case)]
        impl<F, Fut, B, $head> Handler<($head,)> for F
        where
            F: Fn(Request<Body>, $head) -> Fut + Send + Sync,
            Fut: Future<Output = Result<Response<B>, Error>> + Send,
            $head: FromRequest + Send,
        {
            type ResponseBody = B;

            async fn call(self, mut req: Request<Body>) -> Result<Response<Self::ResponseBody>, Error> {
                let $head = $head::from_request(&mut req).await?;
                let res = self(req, $head).await?;
                Ok(res)
            }
        }
    };

    ( $head:ident, $($tail:ident),* $(,)? ) => {
        #[async_trait]
        #[allow(non_snake_case)]
        impl<F, Fut, B, $head, $($tail,)*> Handler<($head, $($tail,)*)> for F
        where
            F: Fn(Request<Body>, $head, $($tail,)*) -> Fut + Send + Sync,
            Fut: Future<Output = Result<Response<B>, Error>> + Send,
            $head: FromRequest + Send,
            $( $tail: FromRequest + Send, )*
        {
            type ResponseBody = B;

            async fn call(self, mut req: Request<Body>) -> Result<Response<Self::ResponseBody>, Error> {
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
impl<S, T, B> Handler<T> for Layered<S, T>
where
    S: Service<Request<Body>, Response = Response<B>> + Send,
    S::Error: Into<BoxError>,
    S::Future: Send,
{
    type ResponseBody = B;

    async fn call(self, req: Request<Body>) -> Result<Response<Self::ResponseBody>, Error> {
        self.svc
            .oneshot(req)
            .await
            .map_err(|err| Error::from(err.into()))
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

pub struct HandlerSvc<H, T> {
    handler: H,
    _input: PhantomData<fn() -> T>,
}

impl<H, T> HandlerSvc<H, T> {
    fn new(handler: H) -> Self {
        Self {
            handler,
            _input: PhantomData,
        }
    }
}

impl<H, T> Clone for HandlerSvc<H, T>
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

impl<H, T> Service<Request<Body>> for HandlerSvc<H, T>
where
    H: Handler<T> + Clone + 'static,
    H::ResponseBody: 'static,
{
    type Response = Response<H::ResponseBody>;
    type Error = Error;
    type Future = future::BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // HandlerSvc can only be constructed from async functions which are always ready
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let handler = self.handler.clone();
        Box::pin(Handler::call(handler, req))
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
            let value = serde_urlencoded::from_str(query)?;
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
                .map_err(Error::ConsumeBody)?;
            let value = serde_json::from_slice(&bytes).map_err(Error::DeserializeRequestBody)?;
            Ok(Json(value))
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
        let response =
            response.map(|body| body.map_err(|err| Error::ResponseBody(err.into())).boxed());
        Poll::Ready(Ok(response))
    }
}

impl<R, T> Service<T> for App<R>
where
    R: Service<T>,
    R::Error: Into<Error>,
{
    type Response = R::Response;
    type Error = R::Error;
    type Future = R::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO(david): map error to response
        self.router.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: T) -> Self::Future {
        // TODO(david): map error to response
        self.router.call(req)
    }
}

impl<R, T> Service<T> for RouteBuilder<R>
where
    App<R>: Service<T>,
    <App<R> as Service<T>>::Error: Into<Error>,
{
    type Response = <App<R> as Service<T>>::Response;
    type Error = <App<R> as Service<T>>::Error;
    type Future = <App<R> as Service<T>>::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO(david): map error to response
        self.app.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: T) -> Self::Future {
        // TODO(david): map error to response
        self.app.call(req)
    }
}

#[cfg(test)]
mod tests {
    #![allow(warnings)]
    use super::*;
    use hyper::Server;
    use std::time::Duration;
    use std::{fmt, net::SocketAddr};
    use tower::{
        layer::util::Identity, make::Shared, service_fn, timeout::TimeoutLayer, ServiceBuilder,
    };
    use tower_http::trace::TraceLayer;

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

        let mut app = app()
            .at("/")
            .get(root.layer(TimeoutLayer::new(Duration::from_secs(30))))
            .at("/users")
            .get(|_: Request<Body>, pagination: Query<Pagination>| async {
                let pagination = pagination.into_inner();
                assert_eq!(pagination.page, 1);
                assert_eq!(pagination.per_page, 30);

                Ok::<_, Error>(Response::new(Body::from("users#index")))
            })
            .post(|_: Request<Body>, payload: Json<UsersCreate>| async {
                let payload = payload.into_inner();
                assert_eq!(payload.username, "bob");

                Ok::<_, Error>(Response::new(Body::from("users#create")))
            })
            .at("/service")
            .get_service(service_fn(root));

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
                    .method(Method::POST)
                    .uri("/users")
                    .body(Body::from(r#"{ "username": "bob" }"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_to_string(res).await, "users#create");
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
        let app = app().at("/").get(|_: Request<Body>| async {
            Ok::<_, Error>(Response::new(Body::from("Hello, World!")))
        });

        let app = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .service(app);

        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
        let server = Server::bind(&addr).serve(Shared::new(app));
        server.await.unwrap();
    }
}
