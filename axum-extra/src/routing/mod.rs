//! Additional types for defining routes.

use axum::Router;

mod resource;

pub use self::resource::Resource;

/// Extension trait that adds additional methods to [`Router`].
pub trait RouterExt<B>: sealed::Sealed {
    /// Add a [`Resource`] to the router.
    ///
    /// See [`Resource`] for more details.
    fn resource<F>(self, name: &str, f: F) -> Self
    where
        F: FnOnce(resource::Resource<B>) -> resource::Resource<B>;
}

impl<B> RouterExt<B> for Router<B> {
    fn resource<F>(self, name: &str, f: F) -> Self
    where
        F: FnOnce(resource::Resource<B>) -> resource::Resource<B>,
    {
        f(resource::Resource {
            name: name.to_owned(),
            router: self,
        })
        .router
    }
}

mod sealed {
    pub trait Sealed {}
    impl<B> Sealed for axum::Router<B> {}
}
