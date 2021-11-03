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
use okapi::openapi3::{Components, Operation, PathItem};
use tower_service::Service;

// NOTE: could we make axum main method routers work like this? Its more generics but conceptually
// simpler imo
// Might be worth it to consider a redesign of axum's method router such that we wouldn't have to
// write all this and could instead wrap it, similarly to how we're wrapping `Router`
// TODO(david): impl fmt::Debug
pub struct MethodRouter<Delete, Get, Head, Options, Patch, Post, Put, Trace, B = Body> {
    pub(crate) delete: Option<WithOperation<Delete>>,
    pub(crate) get: Option<WithOperation<Get>>,
    pub(crate) head: Option<WithOperation<Head>>,
    pub(crate) options: Option<WithOperation<Options>>,
    pub(crate) patch: Option<WithOperation<Patch>>,
    pub(crate) post: Option<WithOperation<Post>>,
    pub(crate) put: Option<WithOperation<Put>>,
    pub(crate) trace: Option<WithOperation<Trace>>,
    pub(crate) _marker: PhantomData<fn() -> B>,
}

#[derive(Clone, Default)]
pub(crate) struct WithOperation<S> {
    svc: S,
    operation: Operation,
    components: Components,
}

impl<H, B, T> WithOperation<IntoService<H, B, T>> {
    fn new(handler: H, id: impl Into<String>) -> Self
    where
        H: Handler<B, T> + ToOperation<T>,
    {
        let mut components = Default::default();
        let mut operation = handler.to_operation(&mut components);
        operation.operation_id = Some(id.into());
        Self {
            svc: handler.into_service(),
            operation,
            components,
        }
    }
}

impl<T> WithOperation<T> {
    pub(crate) fn to_inner(&self) -> (Operation, Components) {
        (self.operation.clone(), self.components.clone())
    }
}

impl<Delete, Get, Head, Options, Patch, Post, Put, Trace, B> Clone
    for MethodRouter<Delete, Get, Head, Options, Patch, Post, Put, Trace, B>
where
    Delete: Clone,
    Get: Clone,
    Head: Clone,
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
    B,
>
where
    H: Handler<B, T> + ToOperation<T>,
{
    MethodRouter {
        delete: Default::default(),
        get: Some(WithOperation::new(handler, id)),
        head: Default::default(),
        options: Default::default(),
        patch: Default::default(),
        post: Default::default(),
        put: Default::default(),
        trace: Default::default(),
        _marker: PhantomData,
    }
}

impl<Delete, Get, Head, Options, Patch, Post, Put, Trace, B>
    MethodRouter<Delete, Get, Head, Options, Patch, Post, Put, Trace, B>
{
    pub fn post<H, T>(
        self,
        id: impl Into<String>,
        handler: H,
    ) -> MethodRouter<Delete, Get, Head, Options, Patch, IntoService<H, B, T>, Put, Trace, B>
    where
        H: Handler<B, T> + ToOperation<T>,
    {
        MethodRouter {
            delete: self.delete,
            get: self.get,
            head: self.head,
            options: self.options,
            patch: self.patch,
            post: Some(WithOperation::new(handler, id)),
            put: self.put,
            trace: self.trace,
            _marker: PhantomData,
        }
    }
}

impl<Delete, Get, Head, Options, Patch, Post, Put, Trace, B> Service<Request<B>>
    for MethodRouter<Delete, Get, Head, Options, Patch, Post, Put, Trace, B>
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
