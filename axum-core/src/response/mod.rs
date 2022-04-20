//! Types and traits for generating responses.
//!
//! See [`axum::response`] for more details.
//!
//! [`axum::response`]: https://docs.rs/axum/latest/axum/response/index.html

use crate::body::BoxBody;

mod into_response;
mod into_response_parts;

pub use self::{
    into_response::IntoResponse,
    into_response_parts::{IntoResponseParts, ResponseParts, TryIntoHeaderError},
};

/// Type alias for [`http::Response`] whose body type defaults to [`BoxBody`], the most common body
/// type used with axum.
pub type Response<T = BoxBody> = http::Response<T>;

/// A flexible [IntoResponse]-based result type
///
/// All types which implement [IntoResponse] can be converted to an [Error].
/// This makes it useful as a general error type for functions which combine
/// multiple distinct error types but all of which implement [IntoResponse].
///
/// For example, note that the error types below differ. However, both can be
/// used with the [Result], and therefore the `?` operator, since they both
/// implement [IntoResponse].
///
/// ```no_run
/// use axum::{
///     response::{IntoResponse, Response, ResultResponse},
///     http::StatusCode,
/// };
///
/// fn handler() -> ResultResponse<&'static str> {
///     Err((StatusCode::NOT_FOUND, "not found"))?;
///     Err(StatusCode::BAD_REQUEST)?;
///     Ok("ok")
/// }
/// ```
pub type ResultResponse<T> = std::result::Result<T, ErrorResponse>;

impl<T> IntoResponse for ResultResponse<T>
where
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        match self {
            Ok(ok) => ok.into_response(),
            Err(err) => err.0,
        }
    }
}

/// An [IntoResponse]-based error type
///
/// See [ResultResponse] for more details.
#[derive(Debug)]
pub struct ErrorResponse(Response);

impl<T> From<T> for ErrorResponse
where
    T: IntoResponse,
{
    fn from(value: T) -> Self {
        Self(value.into_response())
    }
}
