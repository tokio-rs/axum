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
use openapiv3::{Operation, PathItem};
use tower_service::Service;

// NOTE: could we make axum main method routers work like this? Its more generics but conceptually
// simpler imo
// Might be worth it to consider a redesign of axum's method router such that we wouldn't have to
// write all this and could instead wrap it, similarly to how we're wrapping `Router`
// TODO(david): impl fmt::Debug
pub struct MethodRouter<Delete, Get, Head, On, Options, Patch, Post, Put, Trace, B = Body> {
    delete: Delete,
    delete_operation: Option<Operation>,
    get: Get,
    get_operation: Option<Operation>,
    head: Head,
    head_operation: Option<Operation>,
    on: On,
    on_operation: Option<Operation>,
    options: Options,
    options_operation: Option<Operation>,
    patch: Patch,
    patch_operation: Option<Operation>,
    post: Post,
    post_operation: Option<Operation>,
    put: Put,
    put_operation: Option<Operation>,
    trace: Trace,
    trace_operation: Option<Operation>,
    _marker: PhantomData<fn() -> B>,
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
            delete_operation: self.delete_operation.clone(),
            get: self.get.clone(),
            get_operation: self.get_operation.clone(),
            head: self.head.clone(),
            head_operation: self.head_operation.clone(),
            on: self.on.clone(),
            on_operation: self.on_operation.clone(),
            options: self.options.clone(),
            options_operation: self.options_operation.clone(),
            patch: self.patch.clone(),
            patch_operation: self.patch_operation.clone(),
            post: self.post.clone(),
            post_operation: self.post_operation.clone(),
            put: self.put.clone(),
            put_operation: self.put_operation.clone(),
            trace: self.trace.clone(),
            trace_operation: self.trace_operation.clone(),
            _marker: PhantomData,
        }
    }
}

pub fn get<H, B, T>(
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
        delete_operation: Default::default(),
        get_operation: Some(handler.to_operation()),
        get: handler.into_service(),
        head: Default::default(),
        head_operation: Default::default(),
        on: Default::default(),
        on_operation: Default::default(),
        options: Default::default(),
        options_operation: Default::default(),
        patch: Default::default(),
        patch_operation: Default::default(),
        post: Default::default(),
        post_operation: Default::default(),
        put: Default::default(),
        put_operation: Default::default(),
        trace: Default::default(),
        trace_operation: Default::default(),
        _marker: PhantomData,
    }
}

impl<Delete, Get, Head, On, Options, Patch, Post, Put, Trace, B>
    MethodRouter<Delete, Get, Head, On, Options, Patch, Post, Put, Trace, B>
{
    pub fn post<H, T>(
        self,
        handler: H,
    ) -> MethodRouter<Delete, Get, Head, On, Options, Patch, IntoService<H, B, T>, Put, Trace, B>
    where
        H: Handler<B, T> + ToOperation<T>,
    {
        MethodRouter {
            delete: self.delete,
            delete_operation: self.delete_operation,
            get: self.get,
            get_operation: self.get_operation,
            head: self.head,
            head_operation: self.head_operation,
            on: self.on,
            on_operation: self.on_operation,
            options: self.options,
            options_operation: self.options_operation,
            patch: self.patch,
            patch_operation: self.patch_operation,
            post_operation: Some(handler.to_operation()),
            post: handler.into_service(),
            put: self.put,
            put_operation: self.put_operation,
            trace: self.trace,
            trace_operation: self.trace_operation,
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
            get: self.get_operation.clone(),
            put: self.put_operation.clone(),
            post: self.post_operation.clone(),
            delete: self.delete_operation.clone(),
            options: self.options_operation.clone(),
            head: self.head_operation.clone(),
            patch: self.patch_operation.clone(),
            trace: self.trace_operation.clone(),
            servers: Default::default(),
            parameters: Default::default(),
            extensions: Default::default(),
        }
    }
}
