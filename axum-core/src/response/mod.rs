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
/// use axum_core::response::{IntoResponse, Response, Result};
/// use http::StatusCode;
///
/// fn foo() -> Result<&'static str> {
///     Err((StatusCode::NOT_FOUND, "not found"))?;
///     Err(StatusCode::BAD_REQUEST)?;
///     Ok("ok")
/// }
///
/// fn main() {
///     let response: Response = foo().into_response();
/// }
/// ```
pub type Result<T> = std::result::Result<T, Error>;

impl<T: IntoResponse> IntoResponse for Result<T> {
    #[inline]
    fn into_response(self) -> Response {
        match self {
            Ok(ok) => ok.into_response(),
            Err(err) => err.0,
        }
    }
}

/// An [IntoResponse]-based error type
///
/// See [Result] for more details.
#[derive(Debug)]
pub struct Error(Response);

impl<T: IntoResponse> From<T> for Error {
    #[inline]
    fn from(value: T) -> Self {
        Self(value.into_response())
    }
}
