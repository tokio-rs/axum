use std::convert::Infallible;

use super::Handler;
use crate::routing::Route;

pub(crate) struct BoxedHandler<S, B, E = Infallible>(Box<dyn ErasedHandler<S, B, E>>);

impl<S, B> BoxedHandler<S, B>
where
    S: Clone + Send + Sync + 'static,
    B: Send + 'static,
{
    pub(crate) fn new<H, T>(handler: H) -> Self
    where
        H: Handler<T, S, B>,
        T: 'static,
    {
        Self(Box::new(MakeErasedHandler {
            handler,
            into_route: |handler, state| Route::new(Handler::with_state(handler, state)),
        }))
    }
}

impl<S, B, E> BoxedHandler<S, B, E> {
    pub(crate) fn map<F, B2, E2>(self, f: F) -> BoxedHandler<S, B2, E2>
    where
        S: 'static,
        B: 'static,
        E: 'static,
        F: FnOnce(Route<B, E>) -> Route<B2, E2> + Clone + Send + 'static,
        B2: 'static,
        E2: 'static,
    {
        BoxedHandler(Box::new(Map {
            handler: self.0,
            layer: Box::new(f),
        }))
    }

    pub(crate) fn into_route(self, state: S) -> Route<B, E> {
        self.0.into_route(state)
    }
}

impl<S, B, E> Clone for BoxedHandler<S, B, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

trait ErasedHandler<S, B, E = Infallible>: Send {
    fn clone_box(&self) -> Box<dyn ErasedHandler<S, B, E>>;

    fn into_route(self: Box<Self>, state: S) -> Route<B, E>;
}

struct MakeErasedHandler<H, S, B> {
    handler: H,
    into_route: fn(H, S) -> Route<B>,
}

impl<H, S, B> ErasedHandler<S, B> for MakeErasedHandler<H, S, B>
where
    H: Clone + Send + 'static,
    S: 'static,
    B: 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedHandler<S, B>> {
        Box::new(self.clone())
    }

    fn into_route(self: Box<Self>, state: S) -> Route<B> {
        (self.into_route)(self.handler, state)
    }
}

impl<H: Clone, S, B> Clone for MakeErasedHandler<H, S, B> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            into_route: self.into_route,
        }
    }
}

struct Map<S, B, E, B2, E2> {
    handler: Box<dyn ErasedHandler<S, B, E>>,
    layer: Box<dyn LayerFn<B, E, B2, E2>>,
}

impl<S, B, E, B2, E2> ErasedHandler<S, B2, E2> for Map<S, B, E, B2, E2>
where
    S: 'static,
    B: 'static,
    E: 'static,
    B2: 'static,
    E2: 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedHandler<S, B2, E2>> {
        Box::new(Self {
            handler: self.handler.clone_box(),
            layer: self.layer.clone_box(),
        })
    }

    fn into_route(self: Box<Self>, state: S) -> Route<B2, E2> {
        (self.layer)(self.handler.into_route(state))
    }
}

trait LayerFn<B, E, B2, E2>: FnOnce(Route<B, E>) -> Route<B2, E2> + Send {
    fn clone_box(&self) -> Box<dyn LayerFn<B, E, B2, E2>>;
}

impl<F, B, E, B2, E2> LayerFn<B, E, B2, E2> for F
where
    F: FnOnce(Route<B, E>) -> Route<B2, E2> + Clone + Send + 'static,
{
    fn clone_box(&self) -> Box<dyn LayerFn<B, E, B2, E2>> {
        Box::new(self.clone())
    }
}
