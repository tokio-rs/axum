use std::{
    convert::Infallible,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use crate::{ToOperation, ToPathItem};
use axum::{
    body::{Body, BoxBody},
    handler::{Handler, IntoService},
    http::{Request, Response},
    routing::MethodNotAllowed,
};
use okapi::openapi3::{Operation, PathItem};
use tower_service::Service;

// NOTE: could we make axum main method routers work like this? Its more generics but conceptually
// simpler imo
// Might be worth it to consider a redesign of axum's method router such that we wouldn't have to
// write all this and could instead wrap it, similarly to how we're wrapping `Router`
// TODO(david): impl fmt::Debug
pub struct MethodRouter<Delete, Get, Head, On, Options, Patch, Post, Put, Trace, B = Body> {
    delete: Option<WithOperation<Delete>>,
    get: Option<WithOperation<Get>>,
    head: Option<WithOperation<Head>>,
    on: Option<WithOperation<On>>,
    options: Option<WithOperation<Options>>,
    patch: Option<WithOperation<Patch>>,
    post: Option<WithOperation<Post>>,
    put: Option<WithOperation<Put>>,
    trace: Option<WithOperation<Trace>>,
    _marker: PhantomData<fn() -> B>,
}

#[derive(Clone, Default)]
struct WithOperation<T> {
    svc: T,
    operation: Operation,
}

impl<T> WithOperation<T> {
    fn get_operation(&self) -> Operation {
        self.operation.clone()
    }
}

fn operation_with_id<H, T>(id: impl Into<String>, handler: &H) -> Operation
where
    H: ToOperation<T>,
{
    let mut op = handler.to_operation();
    op.operation_id = Some(id.into());
    op
}

impl<Delete, Get, Head, On, Options, Patch, Post, Put, Trace, B> Clone
    for MethodRouter<Delete, Get, Head, On, Options, Patch, Post, Put, Trace, B>
where
    Delete: Clone,
    Get: Clone,
    Head: Clone,
    On: Clone,
    Options: Clone,
    Patch: Clone,
    Post: Clone,
    Put: Clone,
    Trace: Clone,
{
    fn clone(&self) -> Self {
        Self {
            delete: self.delete.clone(),
            get: self.get.clone(),
            head: self.head.clone(),
            on: self.on.clone(),
            options: self.options.clone(),
            patch: self.patch.clone(),
            post: self.post.clone(),
            put: self.put.clone(),
            trace: self.trace.clone(),
            _marker: PhantomData,
        }
    }
}

pub fn get<H, B, T>(
    id: impl Into<String>,
    handler: H,
) -> MethodRouter<
    MethodNotAllowed,
    IntoService<H, B, T>,
    MethodNotAllowed,
    MethodNotAllowed,
    MethodNotAllowed,
    MethodNotAllowed,
    MethodNotAllowed,
    MethodNotAllowed,
    MethodNotAllowed,
    B,
>
where
    H: Handler<B, T> + ToOperation<T>,
{
    MethodRouter {
        delete: Default::default(),
        get: Some(WithOperation {
            operation: operation_with_id(id, &handler),
            svc: handler.into_service(),
        }),
        head: Default::default(),
        on: Default::default(),
        options: Default::default(),
        patch: Default::default(),
        post: Default::default(),
        put: Default::default(),
        trace: Default::default(),
        _marker: PhantomData,
    }
}

impl<Delete, Get, Head, On, Options, Patch, Post, Put, Trace, B>
    MethodRouter<Delete, Get, Head, On, Options, Patch, Post, Put, Trace, B>
{
    pub fn post<H, T>(
        self,
        id: impl Into<String>,
        handler: H,
    ) -> MethodRouter<Delete, Get, Head, On, Options, Patch, IntoService<H, B, T>, Put, Trace, B>
    where
        H: Handler<B, T> + ToOperation<T>,
    {
        MethodRouter {
            delete: self.delete,
            get: self.get,
            head: self.head,
            on: self.on,
            options: self.options,
            patch: self.patch,
            post: Some(WithOperation {
                operation: operation_with_id(id, &handler),
                svc: handler.into_service(),
            }),
            put: self.put,
            trace: self.trace,
            _marker: PhantomData,
        }
    }
}

impl<Delete, Get, Head, On, Options, Patch, Post, Put, Trace, B> Service<Request<B>>
    for MethodRouter<Delete, Get, Head, On, Options, Patch, Post, Put, Trace, B>
where
    B: Send + 'static,
{
    type Response = Response<BoxBody>;
    // TODO(david): error should be generic
    type Error = Infallible;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: Request<B>) -> Self::Future {
        // TODO(david): implement this, will require a future with tons of generics and we just
        // care about things type checking so far
        todo!()
    }
}

impl<Delete, Get, Head, On, Options, Patch, Post, Put, Trace, B> ToPathItem
    for MethodRouter<Delete, Get, Head, On, Options, Patch, Post, Put, Trace, B>
{
    fn to_path_item(&self) -> PathItem {
        PathItem {
            get: self.get.as_ref().map(WithOperation::get_operation),
            put: self.put.as_ref().map(WithOperation::get_operation),
            post: self.post.as_ref().map(WithOperation::get_operation),
            delete: self.delete.as_ref().map(WithOperation::get_operation),
            options: self.options.as_ref().map(WithOperation::get_operation),
            head: self.head.as_ref().map(WithOperation::get_operation),
            patch: self.patch.as_ref().map(WithOperation::get_operation),
            trace: self.trace.as_ref().map(WithOperation::get_operation),
            ..Default::default()
        }
    }
}
