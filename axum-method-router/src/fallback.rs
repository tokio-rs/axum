// when this is moved into axum as part of 0.4 we should unify this with `axum::routing::Fallback`
// (currently private)

use super::MethodRoute;
use std::fmt;

pub(crate) enum Fallback<B, E> {
    Default(MethodRoute<B, E>),
    Custom(MethodRoute<B, E>),
}

impl<B, E> Clone for Fallback<B, E> {
    fn clone(&self) -> Self {
        match self {
            Fallback::Default(inner) => Fallback::Default(inner.clone()),
            Fallback::Custom(inner) => Fallback::Custom(inner.clone()),
        }
    }
}

impl<B, E> fmt::Debug for Fallback<B, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default(inner) => f.debug_tuple("Default").field(inner).finish(),
            Self::Custom(inner) => f.debug_tuple("Custom").field(inner).finish(),
        }
    }
}

impl<B, E> Fallback<B, E> {
    pub(crate) fn map<F, B2, E2>(self, f: F) -> Fallback<B2, E2>
    where
        F: FnOnce(MethodRoute<B, E>) -> MethodRoute<B2, E2>,
    {
        match self {
            Fallback::Default(inner) => Fallback::Default(f(inner)),
            Fallback::Custom(inner) => Fallback::Custom(f(inner)),
        }
    }
}
