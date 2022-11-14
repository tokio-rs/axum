use std::convert::Infallible;

use crate::{body::HttpBody, handler::Handler, routing::Route, Router};

pub(crate) struct BoxedIntoRoute<S, B, E>(Box<dyn ErasedIntoRoute<S, B, E>>);

impl<S, B> BoxedIntoRoute<S, B, Infallible>
where
    S: Clone + Send + Sync + 'static,
    B: Send + 'static,
{
    pub(crate) fn from_handler<H, T>(handler: H) -> Self
    where
        H: Handler<T, S, B>,
        T: 'static,
    {
        Self(Box::new(MakeErasedHandler {
            handler,
            into_route: |handler, state| Route::new(Handler::with_state(handler, state)),
        }))
    }

    pub(crate) fn from_router(router: Router<S, B>) -> Self
    where
        B: HttpBody + Send + 'static,
        S: Clone + Send + Sync + 'static,
    {
        Self(Box::new(MakeErasedRouter {
            router,
            into_route: |router, _state| Route::new(router.into_service()),
        }))
    }
}

impl<S, B, E> BoxedIntoRoute<S, B, E> {
    pub(crate) fn map<F, B2, E2>(self, f: F) -> BoxedIntoRoute<S, B2, E2>
    where
        S: 'static,
        B: 'static,
        E: 'static,
        F: FnOnce(Route<B, E>) -> Route<B2, E2> + Clone + Send + 'static,
        B2: 'static,
        E2: 'static,
    {
        BoxedIntoRoute(Box::new(Map {
            inner: self.0,
            layer: Box::new(f),
        }))
    }

    pub(crate) fn into_route(self, state: S) -> Route<B, E> {
        self.0.into_route(state)
    }

    pub(crate) fn inherit_fallback(&mut self, fallback: Route<B>) {
        self.0.inherit_fallback(fallback);
    }
}

impl<S, B, E> Clone for BoxedIntoRoute<S, B, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

pub(crate) trait ErasedIntoRoute<S, B, E>: Send {
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<S, B, E>>;

    fn into_route(self: Box<Self>, state: S) -> Route<B, E>;

    fn inherit_fallback(&mut self, fallback: Route<B>);
}

pub(crate) struct MakeErasedHandler<H, S, B> {
    pub(crate) handler: H,
    pub(crate) into_route: fn(H, S) -> Route<B>,
}

impl<H, S, B> ErasedIntoRoute<S, B, Infallible> for MakeErasedHandler<H, S, B>
where
    H: Clone + Send + 'static,
    S: 'static,
    B: 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<S, B, Infallible>> {
        Box::new(self.clone())
    }

    fn into_route(self: Box<Self>, state: S) -> Route<B> {
        (self.into_route)(self.handler, state)
    }

    fn inherit_fallback(&mut self, _fallback: Route<B>) {
        // handlers don't have fallbacks, nothing to do here
    }
}

impl<H, S, B> Clone for MakeErasedHandler<H, S, B>
where
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            into_route: self.into_route,
        }
    }
}

pub(crate) struct MakeErasedRouter<S, B> {
    pub(crate) router: Router<S, B>,
    pub(crate) into_route: fn(Router<S, B>, S) -> Route<B>,
}

impl<S, B> ErasedIntoRoute<S, B, Infallible> for MakeErasedRouter<S, B>
where
    S: Clone + Send + 'static,
    B: 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<S, B, Infallible>> {
        Box::new(self.clone())
    }

    fn into_route(self: Box<Self>, state: S) -> Route<B> {
        (self.into_route)(self.router, state)
    }

    fn inherit_fallback(&mut self, fallback: Route<B>) {
        self.router.inherit_fallback(fallback);
    }
}

impl<S, B> Clone for MakeErasedRouter<S, B>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            router: self.router.clone(),
            into_route: self.into_route,
        }
    }
}

pub(crate) struct Map<S, B, E, B2, E2> {
    pub(crate) inner: Box<dyn ErasedIntoRoute<S, B, E>>,
    pub(crate) layer: Box<dyn LayerFn<B, E, B2, E2>>,
}

impl<S, B, E, B2, E2> ErasedIntoRoute<S, B2, E2> for Map<S, B, E, B2, E2>
where
    S: 'static,
    B: 'static,
    E: 'static,
    B2: 'static,
    E2: 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<S, B2, E2>> {
        Box::new(Self {
            inner: self.inner.clone_box(),
            layer: self.layer.clone_box(),
        })
    }

    fn into_route(self: Box<Self>, state: S) -> Route<B2, E2> {
        (self.layer)(self.inner.into_route(state))
    }

    fn inherit_fallback(&mut self, fallback: Route<B2>) {
        // Ideally we'd be able to do this but that doesn't work since `inner` uses body type `B`
        // whereas the fallback has `B2`.
        //
        // Using `B2` makes sense since thats the type after the layer has been applied.
        //
        // However we cannot apply the layer because then we get a `Route` which we cannot
        // add fallbacks to :(
        self.inner.inherit_fallback(fallback);
    }
}

pub(crate) trait LayerFn<B, E, B2, E2>: FnOnce(Route<B, E>) -> Route<B2, E2> + Send {
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
