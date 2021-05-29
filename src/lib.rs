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
use futures_util::future;
use http::{Method, Request, Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    future::Future,
    marker::PhantomData,
    task::{Context, Poll},
};
use tower::{Service, ServiceExt};

pub use hyper::body::Body;

pub fn app() -> App<EmptyRouter> {
    App {
        router: EmptyRouter(()),
    }
}

#[derive(Clone)]
pub struct App<R> {
    router: R,
}

impl<R> App<R> {
    pub fn at(self, route_spec: &str) -> RouteBuilder<R> {
        RouteBuilder {
            app: self,
            route_spec: Bytes::copy_from_slice(route_spec.as_bytes()),
        }
    }
}

pub struct RouteBuilder<R> {
    app: App<R>,
    route_spec: Bytes,
}

impl<R> RouteBuilder<R> {
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

    pub fn at(self, route_spec: &str) -> Self {
        self.app.at(route_spec)
    }

    pub fn into_service(self) -> App<R> {
        self.app
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
            },
        };

        RouteBuilder {
            app: new_app,
            route_spec: self.route_spec,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {}

#[async_trait]
pub trait Handler<Out> {
    async fn call(self, req: Request<Body>) -> Result<Response<Body>, Error>;
}

#[async_trait]
#[allow(non_snake_case)]
impl<F, Fut> Handler<()> for F
where
    F: Fn(Request<Body>) -> Fut + Send + Sync,
    Fut: Future<Output = Result<Response<Body>, Error>> + Send,
{
    async fn call(self, req: Request<Body>) -> Result<Response<Body>, Error> {
        let res = self(req).await?;
        Ok(res)
    }
}

#[async_trait]
#[allow(non_snake_case)]
impl<F, Fut, T1> Handler<(T1,)> for F
where
    F: Fn(Request<Body>, T1) -> Fut + Send + Sync,
    Fut: Future<Output = Result<Response<Body>, Error>> + Send,
    T1: FromRequest + Send,
{
    async fn call(self, mut req: Request<Body>) -> Result<Response<Body>, Error> {
        let T1 = T1::from_request(&mut req).await;
        let res = self(req, T1).await?;
        Ok(res)
    }
}

#[async_trait]
#[allow(non_snake_case)]
impl<F, Fut, T1, T2> Handler<(T1, T2)> for F
where
    F: Fn(Request<Body>, T1, T2) -> Fut + Send + Sync,
    Fut: Future<Output = Result<Response<Body>, Error>> + Send,
    T1: FromRequest + Send,
    T2: FromRequest + Send,
{
    async fn call(self, mut req: Request<Body>) -> Result<Response<Body>, Error> {
        let T1 = T1::from_request(&mut req).await;
        let T2 = T2::from_request(&mut req).await;
        let res = self(req, T1, T2).await?;
        Ok(res)
    }
}

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
{
    type Response = Response<Body>;
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

#[async_trait]
pub trait FromRequest: Sized {
    async fn from_request(req: &mut Request<Body>) -> Self;
}

pub struct Query<T>(Result<T, QueryError>);

impl<T> Query<T> {
    pub fn into_inner(self) -> Result<T, QueryError> {
        self.0
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum QueryError {
    #[error("URI contained no query string")]
    Missing,
    #[error("failed to deserialize query string")]
    Deserialize(#[from] serde_urlencoded::de::Error),
}

#[async_trait]
impl<T> FromRequest for Query<T>
where
    T: DeserializeOwned,
{
    async fn from_request(req: &mut Request<Body>) -> Self {
        let result = (|| {
            let query = req.uri().query().ok_or(QueryError::Missing)?;
            let value = serde_urlencoded::from_str(query)?;
            Ok(value)
        })();
        Query(result)
    }
}

pub struct Json<T>(Result<T, JsonError>);

impl<T> Json<T> {
    pub fn into_inner(self) -> Result<T, JsonError> {
        self.0
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum JsonError {
    #[error("failed to consume the body")]
    ConsumeBody(#[from] hyper::Error),
    #[error("failed to deserialize the body")]
    Deserialize(#[from] serde_json::Error),
}

#[async_trait]
impl<T> FromRequest for Json<T>
where
    T: DeserializeOwned,
{
    async fn from_request(req: &mut Request<Body>) -> Self {
        // TODO(david): require the body to have `content-type: application/json`

        let body = std::mem::take(req.body_mut());

        let result = async move {
            let bytes = hyper::body::to_bytes(body).await?;
            let value = serde_json::from_slice(&bytes)?;
            Ok(value)
        }
        .await;

        Json(result)
    }
}

#[derive(Clone, Copy)]
pub struct EmptyRouter(());

impl Service<Request<Body>> for EmptyRouter {
    type Response = Response<Body>;
    type Error = Error;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: Request<Body>) -> Self::Future {
        let mut res = Response::new(Body::empty());
        *res.status_mut() = StatusCode::NOT_FOUND;
        future::ready(Ok(res))
    }
}

#[derive(Clone)]
pub struct Route<H, F> {
    handler: H,
    route_spec: RouteSpec,
    fallback: F,
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
    H: Service<Request<Body>, Response = Response<Body>, Error = Error> + Clone + Send + 'static,
    H::Future: Send,
    F: Service<Request<Body>, Response = Response<Body>, Error = Error> + Clone + Send + 'static,
    F::Future: Send,
{
    type Response = Response<Body>;
    type Error = Error;
    type Future = future::BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO(david): do we need to drive readiness in `call`?
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        if self.route_spec.matches(&req) {
            let handler_clone = self.handler.clone();
            let mut handler = std::mem::replace(&mut self.handler, handler_clone);
            Box::pin(async move { handler.ready().await?.call(req).await })
        } else {
            let fallback_clone = self.fallback.clone();
            let mut fallback = std::mem::replace(&mut self.fallback, fallback_clone);
            Box::pin(async move { fallback.ready().await?.call(req).await })
        }
    }
}

impl<R> Service<Request<Body>> for App<R>
where
    R: Service<Request<Body>, Response = Response<Body>, Error = Error> + Clone,
{
    type Response = Response<Body>;
    type Error = Error;
    type Future = R::Future;

    // TODO(david): handle backpressure
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.router.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.router.call(req)
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
            .post(users_create)
            .into_service();

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
