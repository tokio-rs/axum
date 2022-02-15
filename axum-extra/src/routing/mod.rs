//! Additional types for defining routes.

use axum::{body::Body, handler::Handler, Router};

mod resource;

#[cfg(feature = "typed-routing")]
mod typed;

pub use self::resource::Resource;

#[cfg(feature = "typed-routing")]
pub use axum_macros::TypedPath;

#[cfg(feature = "typed-routing")]
pub use self::typed::{
    Any, Delete, FirstTwoElementsAre, Get, Head, OneOf, Options, Patch, Post, Put, Trace,
    TypedMethod, TypedPath,
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

    /// Add a typed route to the router.
    ///
    /// The method and path will be inferred from the first two arguments to the handler function
    /// which must implement [`TypedMethod`] and [`TypedPath`] respectively.
    ///
    /// # Example
    ///
    /// ```rust
    /// use serde::Deserialize;
    /// use axum::{Router, extract::Json};
    /// use axum_extra::routing::{
    ///     TypedPath,
    ///     Get,
    ///     Post,
    ///     Delete,
    ///     RouterExt, // for `Router::typed_*`
    /// };
    ///
    /// // A type safe route with `/users/:id` as its associated path.
    /// #[derive(TypedPath, Deserialize)]
    /// #[typed_path("/users/:id")]
    /// struct UsersMember {
    ///     id: u32,
    /// }
    ///
    /// // A regular handler function that takes `Get` as its first argument and
    /// // `UsersMember` as the second argument and thus creates a typed connection
    /// // between this handler and `GET /users/:id`.
    /// //
    /// // The first argument must implement `TypedMethod` and the second must
    /// // implement `TypedPath`.
    /// async fn users_show(
    ///     _: Get,
    ///     UsersMember { id }: UsersMember,
    /// ) {
    ///     // ...
    /// }
    ///
    /// let app = Router::new()
    ///     // Add our typed route to the router.
    ///     //
    ///     // The method and path will be inferred to `GET /users/:id` since `users_show`'s
    ///     // first argument is `Get` and the second is `UsersMember`.
    ///     .typed_route(users_show)
    ///     .typed_route(users_create)
    ///     .typed_route(users_destroy);
    ///
    /// #[derive(TypedPath)]
    /// #[typed_path("/users")]
    /// struct UsersCollection;
    ///
    /// #[derive(Deserialize)]
    /// struct UsersCreatePayload { /* ... */ }
    ///
    /// async fn users_create(
    ///     _: Post,
    ///     _: UsersCollection,
    ///     // Our handlers can accept other extractors.
    ///     Json(payload): Json<UsersCreatePayload>,
    /// ) {
    ///     // ...
    /// }
    ///
    /// async fn users_destroy(_: Delete, _: UsersCollection) { /* ... */ }
    ///
    /// #
    /// # let app: Router<axum::body::Body> = app;
    /// ```
    #[cfg(feature = "typed-routing")]
    fn typed_route<H, T, M, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstTwoElementsAre<M, P> + 'static,
        M: TypedMethod,
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

    #[cfg(feature = "typed-routing")]
    fn typed_route<H, T, M, P>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: FirstTwoElementsAre<M, P> + 'static,
        M: TypedMethod,
        P: TypedPath,
    {
        self.route(P::PATH, M::apply_method_router(handler))
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
