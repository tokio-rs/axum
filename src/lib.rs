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
use pin_project::pin_project;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{Service, ServiceExt};

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

    pub fn post<F, T>(self, handler_fn: F) -> RouteBuilder<Route<HandlerSvc<F, T>, R>>
    where
        F: Handler<T>,
    {
        self.add_route(handler_fn, Method::POST)
    }

    fn add_route<H, T>(self, handler: H, method: Method) -> RouteBuilder<Route<HandlerSvc<H, T>, R>>
    where
        H: Handler<T>,
    {
        let new_app = App {
            router: Route {
                handler: HandlerSvc {
                    handler,
                    _input: PhantomData,
                },
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

    pub fn post<F, T>(self, handler_fn: F) -> RouteBuilder<Route<HandlerSvc<F, T>, R>>
    where
        F: Handler<T>,
    {
        self.app.at_bytes(self.route_spec).post(handler_fn)
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
}

// TODO(david): make this trait sealed
#[async_trait]
pub trait Handler<Out> {
    type ResponseBody;

    async fn call(self, req: Request<Body>) -> Result<Response<Self::ResponseBody>, Error>;
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

pub struct HandlerSvc<H, T> {
    handler: H,
    _input: PhantomData<fn() -> T>,
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
    type Error = Error;
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
    handler: H,
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
            handler: self.handler.clone(),
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

impl<H, F> Service<Request<Body>> for Route<H, F>
where
    H: Service<Request<Body>, Response = Response<Body>, Error = Error>,
    F: Service<Request<Body>, Response = Response<Body>, Error = Error>,
{
    type Response = Response<Body>;
    type Error = Error;
    type Future = future::Either<H::Future, F::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        loop {
            if !self.handler_ready {
                ready!(self.handler.poll_ready(cx))?;
                self.handler_ready = true;
            }

            if !self.fallback_ready {
                ready!(self.fallback.poll_ready(cx))?;
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
            future::Either::Left(self.handler.call(req))
        } else {
            assert!(
                self.fallback_ready,
                "fallback not ready. Did you forget to call `poll_ready`?"
            );
            self.fallback_ready = false;
            future::Either::Right(self.fallback.call(req))
        }
    }
}

impl<R, T> Service<T> for App<R>
where
    R: Service<T>,
{
    type Response = R::Response;
    type Error = R::Error;
    type Future = R::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.router.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: T) -> Self::Future {
        self.router.call(req)
    }
}

impl<R, T> Service<T> for RouteBuilder<R>
where
    App<R>: Service<T>,
{
    type Response = <App<R> as Service<T>>::Response;
    type Error = <App<R> as Service<T>>::Error;
    type Future = <App<R> as Service<T>>::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.app.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: T) -> Self::Future {
        self.app.call(req)
    }
}

#[cfg(test)]
mod tests {
    #![allow(warnings)]
    use super::*;

    #[tokio::test]
    async fn basic() {
        let mut app = app()
            .at("/")
            .get(root)
            .at("/users")
            .get(users_index)
            .post(users_create);

        let req = Request::builder()
            .method(Method::POST)
            .uri("/users")
            .body(Body::from(r#"{ "username": "bob" }"#))
            .unwrap();

        let res = app.ready().await.unwrap().call(req).await.unwrap();
        let body = body_to_string(res).await;
        dbg!(&body);
    }

    async fn body_to_string(res: Response<Body>) -> String {
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    async fn root(req: Request<Body>) -> Result<Response<Body>, Error> {
        Ok(Response::new(Body::from("Hello, World!")))
    }

    async fn users_index(
        req: Request<Body>,
        pagination: Query<Pagination>,
    ) -> Result<Response<Body>, Error> {
        dbg!(pagination.into_inner());
        Ok(Response::new(Body::from("users#index")))
    }

    #[derive(Debug, Deserialize)]
    struct Pagination {
        page: usize,
        per_page: usize,
    }

    async fn users_create(
        req: Request<Body>,
        payload: Json<UsersCreate>,
    ) -> Result<Response<Body>, Error> {
        dbg!(payload.into_inner());
        Ok(Response::new(Body::from("users#create")))
    }

    #[derive(Debug, Deserialize)]
    struct UsersCreate {
        username: String,
    }
}
