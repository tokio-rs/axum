//! Additional types for defining routes.

use axum::{body::Body, handler::Handler, Router};

mod resource;
mod typed;

pub use self::{
    resource::Resource,
    typed::{FirstElementIs, TypedPath},
};

/// Extension trait that adds additional methods to [`Router`].
pub trait RouterExt<B>: sealed::Sealed {
    /// Add the routes from `T`'s [`HasRoutes::routes`] to this router.
    ///
    /// # Example
    ///
    /// Using [`Resource`] which implements [`HasRoutes`]:
    ///
    /// ```rust
    /// use axum::{Router, routing::get};
    /// use axum_extra::routing::{RouterExt, Resource};
    ///
    /// let app = Router::new()
    ///     .with(
    ///         Resource::named("users")
    ///             .index(|| async {})
    ///             .create(|| async {})
    ///     )
    ///     .with(
    ///         Resource::named("teams").index(|| async {})
    ///     );
    /// # let _: Router<axum::body::Body> = app;
    /// ```
    fn with<T>(self, routes: T) -> Self
    where
        T: HasRoutes<B>;

    /// TODO(david): docs
    fn typed_get<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath;

    /// TODO(david): docs
    fn typed_delete<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath;

    /// TODO(david): docs
    fn typed_head<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath;

    /// TODO(david): docs
    fn typed_options<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath;

    /// TODO(david): docs
    fn typed_patch<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath;

    /// TODO(david): docs
    fn typed_post<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath;

    /// TODO(david): docs
    fn typed_put<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath;

    /// TODO(david): docs
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
    fn with<T>(self, routes: T) -> Self
    where
        T: HasRoutes<B>,
    {
        self.merge(routes.routes())
    }

    fn typed_get<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::get(handler))
    }

    fn typed_delete<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::delete(handler))
    }

    fn typed_head<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::head(handler))
    }

    fn typed_options<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::options(handler))
    }

    fn typed_patch<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::patch(handler))
    }

    fn typed_post<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::post(handler))
    }

    fn typed_put<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::put(handler))
    }

    fn typed_trace<H, T, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstElementIs<P> + 'static,
        P: TypedPath,
    {
        self.route(P::PATH, axum::routing::trace(handler))
    }
}

/// Trait for things that can provide routes.
///
/// Used with [`RouterExt::with`].
pub trait HasRoutes<B = Body> {
    /// Get the routes.
    fn routes(self) -> Router<B>;
}

impl<B> HasRoutes<B> for Router<B> {
    fn routes(self) -> Router<B> {
        self
    }
}

mod sealed {
    pub trait Sealed {}
    impl<B> Sealed for axum::Router<B> {}
}
