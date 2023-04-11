use axum::{
    async_trait,
    extract::{Extension, FromRequestParts},
};
use http::request::Parts;

/// Cache results of other extractors.
///
/// `Cached` wraps another extractor and caches its result in [request extensions].
///
/// This is useful if you have a tree of extractors that share common sub-extractors that
/// you only want to run once, perhaps because they're expensive.
///
/// The cache purely type based so you can only cache one value of each type. The cache is also
/// local to the current request and not reused across requests.
///
/// # Example
///
/// ```rust
/// use axum_extra::extract::Cached;
/// use axum::{
///     async_trait,
///     extract::FromRequestParts,
///     body::BoxBody,
///     response::{IntoResponse, Response},
///     http::{StatusCode, request::Parts},
/// };
///
/// #[derive(Clone)]
/// struct Session { /* ... */ }
///
/// #[async_trait]
/// impl<S> FromRequestParts<S> for Session
/// where
///     S: Send + Sync,
/// {
///     type Rejection = (StatusCode, String);
///
///     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
///         // load session...
///         # unimplemented!()
///     }
/// }
///
/// struct CurrentUser { /* ... */ }
///
/// #[async_trait]
/// impl<S> FromRequestParts<S> for CurrentUser
/// where
///     S: Send + Sync,
/// {
///     type Rejection = Response;
///
///     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
///         // loading a `CurrentUser` requires first loading the `Session`
///         //
///         // by using `Cached<Session>` we avoid extracting the session more than
///         // once, in case other extractors for the same request also loads the session
///         let session: Session = Cached::<Session>::from_request_parts(parts, state)
///             .await
///             .map_err(|err| err.into_response())?
///             .0;
///
///         // load user from session...
///         # unimplemented!()
///     }
/// }
///
/// // handler that extracts the current user and the session
/// //
/// // the session will only be loaded once, even though `CurrentUser`
/// // also loads it
/// async fn handler(
///     current_user: CurrentUser,
///     // we have to use `Cached<Session>` here otherwise the
///     // cached session would not be used
///     Cached(session): Cached<Session>,
/// ) {
///     // ...
/// }
/// ```
///
/// [request extensions]: http::Extensions
#[derive(Debug, Clone, Default)]
pub struct Cached<T>(pub T);

#[derive(Clone)]
struct CachedEntry<T>(T);

#[async_trait]
impl<S, T> FromRequestParts<S> for Cached<T>
where
    S: Send + Sync,
    T: FromRequestParts<S> + Clone + Send + Sync + 'static,
{
    type Rejection = T::Rejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Extension::<CachedEntry<T>>::from_request_parts(parts, state).await {
            Ok(Extension(CachedEntry(value))) => Ok(Self(value)),
            Err(_) => {
                let value = T::from_request_parts(parts, state).await?;
                parts.extensions.insert(CachedEntry(value.clone()));
                Ok(Self(value))
            }
        }
    }
}

axum_core::__impl_deref!(Cached);

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::FromRequestParts, http::Request, routing::get, Router};
    use http::request::Parts;
    use std::{
        convert::Infallible,
        sync::atomic::{AtomicU32, Ordering},
        time::Instant,
    };

    #[tokio::test]
    async fn works() {
        static COUNTER: AtomicU32 = AtomicU32::new(0);

        #[derive(Clone, Debug, PartialEq, Eq)]
        struct Extractor(Instant);

        #[async_trait]
        impl<S> FromRequestParts<S> for Extractor
        where
            S: Send + Sync,
        {
            type Rejection = Infallible;

            async fn from_request_parts(
                _parts: &mut Parts,
                _state: &S,
            ) -> Result<Self, Self::Rejection> {
                COUNTER.fetch_add(1, Ordering::SeqCst);
                Ok(Self(Instant::now()))
            }
        }

        let (mut parts, _) = Request::new(()).into_parts();

        let first = Cached::<Extractor>::from_request_parts(&mut parts, &())
            .await
            .unwrap()
            .0;
        assert_eq!(COUNTER.load(Ordering::SeqCst), 1);

        let second = Cached::<Extractor>::from_request_parts(&mut parts, &())
            .await
            .unwrap()
            .0;
        assert_eq!(COUNTER.load(Ordering::SeqCst), 1);

        assert_eq!(first, second);
    }

    // Not a #[test], we just want to know this compiles
    async fn _last_handler_argument() {
        async fn handler(_: http::Method, _: Cached<http::HeaderMap>) {}
        let _r: Router = Router::new().route("/", get(handler));
    }
}
