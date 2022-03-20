//! Additional types for defining routes.

use axum::{handler::Handler, Router};

mod resource;

#[cfg(feature = "spa")]
mod spa;

#[cfg(feature = "typed-routing")]
mod typed;

pub use self::resource::Resource;

#[cfg(feature = "typed-routing")]
pub use axum_macros::TypedPath;

#[cfg(feature = "typed-routing")]
pub use self::typed::{FirstElementIs, TypedPath};

#[cfg(feature = "spa")]
pub use self::spa::SpaRouter;

/// Extension trait that adds additional methods to [`Router`].
pub trait RouterExt<B>: sealed::Sealed {
    /// Add a typed `GET` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_get<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath;

    /// Add a typed `DELETE` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_delete<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath;

    /// Add a typed `HEAD` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_head<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath;

    /// Add a typed `OPTIONS` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_options<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath;

    /// Add a typed `PATCH` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_patch<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath;

    /// Add a typed `POST` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_post<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath;

    /// Add a typed `PUT` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_put<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath;

    /// Add a typed `TRACE` route to the router.
    ///
    /// The path will be inferred from the first argument to the handler function which must
    /// implement [`TypedPath`].
    ///
    /// See [`TypedPath`] for more details and examples.
    #[cfg(feature = "typed-routing")]
    fn typed_trace<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath;
}

impl<B> RouterExt<B> for Router<B>
where
    B: axum::body::HttpBody + Send + 'static,
{
    #[cfg(feature = "typed-routing")]
    fn typed_get<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::get(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_delete<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::delete(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_head<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::head(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_options<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::options(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_patch<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::patch(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_post<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::post(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_put<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::put(handler))
    }

    #[cfg(feature = "typed-routing")]
    fn typed_trace<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::trace(handler))
    }
}

mod sealed {
    pub trait Sealed {}
    impl<B> Sealed for axum::Router<B> {}
}
