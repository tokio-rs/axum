#![allow(missing_docs, missing_debug_implementations)]

use axum::{body::HttpBody, handler::Handler, routing, Router};
use std::{borrow::Cow, marker::PhantomData};

use super::HasRoutes;

/// ```rust
/// use axum_macros::TypedPath;
///
/// #[derive(TypedPath)]
/// #[typed_path("/users/:id")]
/// struct UsersShow {
///     id: u32,
/// }
/// ```
pub trait TypedPath {
    const PATH: &'static str;
    fn path(&self) -> Cow<'static, str>;
}

pub fn get<H, B, T, P>(handler: H) -> TypedPathRouter<P, B>
where
    H: Handler<T, B>,
    P: TypedPath,
    T: FirstElementIs<P> + 'static,
    B: HttpBody + Send + 'static,
{
    TypedPathRouter {
        router: Router::new().route(P::PATH, routing::get(handler)),
        _path: PhantomData,
    }
}

pub struct TypedPathRouter<P, B> {
    router: Router<B>,
    _path: PhantomData<P>,
}

impl<P, B> TypedPathRouter<P, B>
where
    B: HttpBody + Send + 'static,
    P: TypedPath,
{
    pub fn post<H, T>(mut self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
    {
        self.router = self.router.route(P::PATH, routing::post(handler));
        self
    }
}

impl<P, B> HasRoutes<B> for TypedPathRouter<P, B> {
    fn routes(self) -> Router<B> {
        self.router
    }
}

pub trait FirstElementIs<P> {}
impl<P> FirstElementIs<P> for (P,) {}
impl<P, T1> FirstElementIs<P> for (P, T1) {}
impl<P, T1, T2> FirstElementIs<P> for (P, T1, T2) {}
