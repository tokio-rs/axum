//! Additional types for defining routes.

use axum::{body::Body, Router};

mod resource;

pub use self::resource::Resource;

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
}

impl<B> RouterExt<B> for Router<B>
where
    B: Send + 'static,
{
    fn with<T>(self, routes: T) -> Self
    where
        T: HasRoutes<B>,
    {
        self.merge(routes.routes())
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
