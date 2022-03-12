#![allow(missing_debug_implementations)]

use axum::{
    async_trait,
    body::HttpBody,
    handler::Handler,
    http::Request,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use okapi::openapi3::{Info, OpenApi, Operation};
use std::{future::Future, marker::PhantomData, sync::Arc};

pub struct OpenApiRouter<B> {
    router: Router<B>,
    schema: OpenApi,
}

impl<B> OpenApiRouter<B>
where
    B: HttpBody + Send + 'static,
{
    pub fn new(info: Info) -> Self {
        Self {
            router: Default::default(),
            schema: OpenApi {
                info,
                ..Default::default()
            },
        }
    }

    pub fn get<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: OpenApiHandler<T, B>,
        T: 'static,
    {
        let mut operation = Operation::default();
        handler.clone().to_operation(&mut operation);
        self.schema.paths.entry(path.to_owned()).or_default().get = Some(operation);
        self.router = self.router.route(path, get(handler));
        self
    }

    pub fn post<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: OpenApiHandler<T, B>,
        T: 'static,
    {
        let mut operation = Operation::default();
        handler.clone().to_operation(&mut operation);
        self.schema.paths.entry(path.to_owned()).or_default().post = Some(operation);
        self.router = self.router.route(path, post(handler));
        self
    }
}

pub trait OpenApiHandler<T, B>: Handler<T, B> {
    fn to_operation(self, operation: &mut Operation);

    fn map_operation<F>(self, f: F) -> MapOperation<Self, T, B, F>
    where
        F: FnOnce(&mut Operation),
    {
        MapOperation {
            handler: self,
            f,
            _marker: PhantomData,
        }
    }
}

pub struct MapOperation<H, T, B, F> {
    handler: H,
    f: F,
    _marker: PhantomData<(T, B)>,
}

impl<H, T, B, F> Clone for MapOperation<H, T, B, F>
where
    H: Clone,
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            f: self.f.clone(),
            _marker: PhantomData,
        }
    }
}

impl<H, T, B, F> OpenApiHandler<T, B> for MapOperation<H, T, B, F>
where
    H: OpenApiHandler<T, B> + Handler<T, B>,
    F: FnOnce(&mut Operation) + Clone + Send + 'static,
    T: Send + 'static,
    B: Send + 'static,
{
    fn to_operation(self, operation: &mut Operation) {
        (self.f)(operation);
    }
}

impl<H, T, B, F> Handler<T, B> for MapOperation<H, T, B, F>
where
    H: Handler<T, B>,
    F: Clone + Send + 'static,
    T: Send + 'static,
    B: Send + 'static,
{
    type Future = H::Future;

    fn call(self, req: Request<B>) -> Self::Future {
        self.handler.call(req)
    }
}

impl<F, Fut, Res, B> OpenApiHandler<(), B> for F
where
    F: FnOnce() -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
{
    fn to_operation(self, operation: &mut Operation) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

    #[test]
    fn test_something() {
        async fn users_show() {}

        async fn users_create() {}

        let _router: OpenApiRouter<Body> = OpenApiRouter::new(Info::default())
            .post(
                "/users",
                users_create
                    .map_operation(|_| {})
                    .map_operation(|mut operation| {
                        operation.summary = Some("Create a new user".to_owned());
                    }),
            )
            .get("/users/:id", users_show.map_operation(|_operation| {}));
    }
}
