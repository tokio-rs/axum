//! Types and traits for extracting data from requests.
//!
//! See [`axum::extract`] for more details.
//!
//! [`axum::extract`]: https://docs.rs/axum/latest/axum/extract/index.html

use crate::response::IntoResponse;
use async_trait::async_trait;
use http::{request::Parts, Request};
use std::convert::Infallible;

pub mod rejection;

mod from_ref;
mod request_parts;
mod tuple;

pub use self::from_ref::FromRef;

mod __private {
    #[derive(Debug, Clone, Copy)]
    pub enum Mut {}

    #[derive(Debug, Clone, Copy)]
    pub enum Once {}
}

/// Types that can be created from requests.
///
/// See [`axum::extract`] for more details.
///
/// # What is the `B` type parameter?
///
/// `FromRequest` is generic over the request body (the `B` in
/// [`http::Request<B>`]). This is to allow `FromRequest` to be usable with any
/// type of request body. This is necessary because some middleware change the
/// request body, for example to add timeouts.
///
/// If you're writing your own `FromRequest` that wont be used outside your
/// application, and not using any middleware that changes the request body, you
/// can most likely use `axum::body::Body`.
///
/// If you're writing a library that's intended for others to use, it's recommended
/// to keep the generic type parameter:
///
/// ```rust
/// use axum::{
///     async_trait,
///     extract::{FromRequest, RequestParts},
/// };
///
/// struct MyExtractor;
///
/// #[async_trait]
/// impl<S, B> FromRequest<S, B> for MyExtractor
/// where
///     // these bounds are required by `async_trait`
///     B: Send,
///     S: Send + Sync,
/// {
///     type Rejection = http::StatusCode;
///
///     async fn from_request(req: &mut RequestParts<S, B>) -> Result<Self, Self::Rejection> {
///         // ...
///         # unimplemented!()
///     }
/// }
/// ```
///
/// This ensures your extractor is as flexible as possible.
///
/// [`http::Request<B>`]: http::Request
/// [`axum::extract`]: https://docs.rs/axum/latest/axum/extract/index.html
#[async_trait]
pub trait FromRequestParts<S, B>: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection>;
}

/// TODO
#[async_trait]
pub trait FromRequest<S, B, M = __private::Once>: Sized {
    /// TODO
    type Rejection: IntoResponse;

    /// TODO
    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection>;
}

#[async_trait]
impl<S, B, T> FromRequest<S, B, __private::Mut> for T
where
    B: Send + 'static,
    S: Send + Sync,
    T: FromRequestParts<S, B>,
{
    type Rejection = <Self as FromRequestParts<S, B>>::Rejection;

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        let (mut parts, _) = req.into_parts();
        Self::from_request_parts(&mut parts, state).await
    }
}

#[async_trait]
impl<S, T, B> FromRequestParts<S, B> for Option<T>
where
    T: FromRequestParts<S, B>,
    B: http_body::Body + Send + Unpin,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<T>, Self::Rejection> {
        Ok(T::from_request_parts(parts, state).await.ok())
    }
}

#[async_trait]
impl<S, T, B> FromRequest<S, B> for Option<T>
where
    T: FromRequest<S, B>,
    B: Send + 'static,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: Request<B>, state: &S) -> Result<Option<T>, Self::Rejection> {
        Ok(T::from_request(req, state).await.ok())
    }
}

#[async_trait]
impl<S, T, B> FromRequestParts<S, B> for Result<T, T::Rejection>
where
    T: FromRequestParts<S, B>,
    B: Send,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_request_parts(parts, state).await)
    }
}

#[async_trait]
impl<S, T, B> FromRequest<S, B> for Result<T, T::Rejection>
where
    T: FromRequest<S, B>,
    B: Send + 'static,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_request(req, state).await)
    }
}
