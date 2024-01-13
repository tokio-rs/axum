use async_trait::async_trait;
use http::request::Parts;

use crate::response::IntoResponse;

use super::{private, FromRequest, FromRequestParts, Request};

/// TODO: DOCS
#[async_trait]
pub trait OptionalFromRequestParts<S>: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection>;
}

/// TODO: DOCS
#[async_trait]
pub trait OptionalFromRequest<S, M = private::ViaRequest>: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    async fn from_request(req: Request, state: &S) -> Result<Option<Self>, Self::Rejection>;
}

#[async_trait]
impl<S, T> FromRequestParts<S> for Option<T>
where
    T: OptionalFromRequestParts<S>,
    S: Send + Sync,
{
    type Rejection = T::Rejection;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<T>, Self::Rejection> {
        T::from_request_parts(parts, state).await
    }
}

#[async_trait]
impl<S, T> FromRequest<S> for Option<T>
where
    T: OptionalFromRequest<S>,
    S: Send + Sync,
{
    type Rejection = T::Rejection;

    async fn from_request(req: Request, state: &S) -> Result<Option<T>, Self::Rejection> {
        T::from_request(req, state).await
    }
}
