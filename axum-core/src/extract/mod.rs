//! Types and traits for extracting data from requests.
//!
//! See [`axum::extract`] for more details.
//!
//! [`axum::extract`]: https://docs.rs/axum/0.8/axum/extract/index.html

use crate::{body::Body, response::IntoResponse};
use http::request::Parts;
use std::convert::Infallible;
use std::future::Future;

pub mod rejection;

mod default_body_limit;
mod from_ref;
mod option;
mod request_parts;
mod tuple;

pub(crate) use self::default_body_limit::DefaultBodyLimitKind;
pub use self::{
    default_body_limit::DefaultBodyLimit,
    from_ref::FromRef,
    option::{OptionalFromRequest, OptionalFromRequestParts},
};

/// Type alias for [`http::Request`] whose body type defaults to [`Body`], the most common body
/// type used with axum.
pub type Request<T = Body> = http::Request<T>;

pub mod private {
    #[derive(Debug, Clone, Copy)]
    pub enum ViaParts {}

    #[derive(Debug, Clone, Copy)]
    pub enum ViaStatelessParts {}

    #[derive(Debug, Clone, Copy)]
    pub enum ViaRequest {}

    #[derive(Debug, Clone, Copy)]
    pub enum WithState {}

    #[derive(Debug, Clone, Copy)]
    pub enum Stateless {}
}

/// Types that can be created from request parts.
///
/// Extractors that implement `FromRequestParts` cannot consume the request body and can thus be
/// run in any order for handlers.
///
/// If your extractor needs to consume the request body then you should implement [`FromRequest`]
/// and not [`FromRequestParts`].
///
/// See [`axum::extract`] for more general docs about extractors.
///
/// [`axum::extract`]: https://docs.rs/axum/0.8/axum/extract/index.html
#[rustversion::attr(
    since(1.78),
    diagnostic::on_unimplemented(
        note = "Function argument is not a valid axum extractor. \nSee `https://docs.rs/axum/0.8/axum/extract/index.html` for details",
    )
)]
pub trait FromRequestParts<S, Via = private::WithState>: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send;
}

/// Like `FromRequestParts` but without `State`.
pub trait FromStatelessRequestParts: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    fn from_request_parts(
        parts: &mut Parts,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send;
}

impl<T, S> FromRequestParts<S, private::Stateless> for T
where
    T: FromStatelessRequestParts,
{
    type Rejection = T::Rejection;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        T::from_request_parts(parts)
    }
}

/// Types that can be created from requests.
///
/// Extractors that implement `FromRequest` can consume the request body and can thus only be run
/// once for handlers.
///
/// If your extractor doesn't need to consume the request body then you should implement
/// [`FromRequestParts`] and not [`FromRequest`].
///
/// See [`axum::extract`] for more general docs about extractors.
///
/// [`axum::extract`]: https://docs.rs/axum/0.8/axum/extract/index.html
#[rustversion::attr(
    since(1.78),
    diagnostic::on_unimplemented(
        note = "Function argument is not a valid axum extractor. \nSee `https://docs.rs/axum/0.8/axum/extract/index.html` for details",
    )
)]
pub trait FromRequest<S, M = private::ViaRequest>: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    fn from_request(
        req: Request,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send;
}

impl<S, T> FromRequest<S, private::ViaParts> for T
where
    S: Send + Sync,
    T: FromRequestParts<S, private::WithState>,
{
    type Rejection = <Self as FromRequestParts<S>>::Rejection;

    fn from_request(
        req: Request,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> {
        let (mut parts, _) = req.into_parts();
        async move { Self::from_request_parts(&mut parts, state).await }
    }
}

impl<S, T> FromRequest<S, private::ViaStatelessParts> for T
where
    S: Send + Sync,
    T: FromRequestParts<S, private::Stateless>,
{
    type Rejection = <Self as FromRequestParts<S, private::Stateless>>::Rejection;

    fn from_request(
        req: Request,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> {
        let (mut parts, _) = req.into_parts();
        async move { Self::from_request_parts(&mut parts, state).await }
    }
}


impl<S, T> FromRequestParts<S> for Result<T, T::Rejection>
where
    T: FromRequestParts<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_request_parts(parts, state).await)
    }
}

impl<S, T> FromRequest<S> for Result<T, T::Rejection>
where
    T: FromRequest<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_request(req, state).await)
    }
}
