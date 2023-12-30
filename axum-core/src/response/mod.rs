//! Types and traits for generating responses.
//!
//! See [`axum::response`] for more details.
//!
//! [`axum::response`]: https://docs.rs/axum/0.7/axum/response/index.html

use std::convert::Infallible;

use http::StatusCode;

use crate::body::Body;

mod append_headers;
mod into_response;
mod into_response_parts;

pub use self::{
    append_headers::AppendHeaders,
    into_response::IntoResponse,
    into_response_parts::{IntoResponseParts, ResponseParts, TryIntoHeaderError},
};

/// Type alias for [`http::Response`] whose body type defaults to [`Body`], the most common body
/// type used with axum.
pub type Response<T = Body> = http::Response<T>;

/// An [`IntoResponse`]-based result type that uses [`ErrorResponse`] as the error type.
///
/// All types which implement [`IntoResponse`] can be converted to an [`ErrorResponse`]. This makes
/// it useful as a general purpose error type for functions which combine multiple distinct error
/// types that all implement [`IntoResponse`].
///
/// # Example
///
/// ```
/// use axum::{
///     response::{IntoResponse, Response},
///     http::StatusCode,
/// };
///
/// // two fallible functions with different error types
/// fn try_something() -> Result<(), ErrorA> {
///     // ...
///     # unimplemented!()
/// }
///
/// fn try_something_else() -> Result<(), ErrorB> {
///     // ...
///     # unimplemented!()
/// }
///
/// // each error type implements `IntoResponse`
/// struct ErrorA;
///
/// impl IntoResponse for ErrorA {
///     fn into_response(self) -> Response {
///         // ...
///         # unimplemented!()
///     }
/// }
///
/// enum ErrorB {
///     SomethingWentWrong,
/// }
///
/// impl IntoResponse for ErrorB {
///     fn into_response(self) -> Response {
///         // ...
///         # unimplemented!()
///     }
/// }
///
/// // we can combine them using `axum::response::Result` and still use `?`
/// async fn handler() -> axum::response::Result<&'static str> {
///     // the errors are automatically converted to `ErrorResponse`
///     try_something()?;
///     try_something_else()?;
///
///     Ok("it worked!")
/// }
/// ```
///
/// # As a replacement for `std::result::Result`
///
/// Since `axum::response::Result` has a default error type you only have to specify the `Ok` type:
///
/// ```
/// use axum::{
///     response::{IntoResponse, Response, Result},
///     http::StatusCode,
/// };
///
/// // `Result<T>` automatically uses `ErrorResponse` as the error type.
/// async fn handler() -> Result<&'static str> {
///     try_something()?;
///
///     Ok("it worked!")
/// }
///
/// // You can still specify the error even if you've imported `axum::response::Result`
/// fn try_something() -> Result<(), StatusCode> {
///     // ...
///     # unimplemented!()
/// }
/// ```
pub type Result<T, E = ErrorResponse> = std::result::Result<T, E>;

impl<T> IntoResponse for Result<T>
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

/// An [`IntoResponse`]-based error type
///
/// See [`Result`] for more details.
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

/// ```
/// todo!();
/// ```
#[derive(Copy, Clone, Debug)]
pub struct IntoResponseFailed;

impl IntoResponseParts for IntoResponseFailed {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        res.extensions_mut().insert(self);
        Ok(res)
    }
}

/// Not sure it makes sense to return `IntoResponseFailed` as the whole response. You should
/// probably at least combine it with a status code.
///
/// ```compile_fail
/// fn foo()
/// where
///     axum_core::response::IntoResponseFailed: axum_core::response::IntoResponse,
/// {}
/// ```
#[allow(dead_code)]
fn into_response_failed_doesnt_impl_into_response() {}

/// Override all status codes regardless if [`IntoResponseFailed`] is used or not.
///
/// See the docs for [`IntoResponseFailed`] for more details.
#[derive(Debug, Copy, Clone, Default)]
pub struct OverrideAllStatusCodes(pub StatusCode);

impl IntoResponse for OverrideAllStatusCodes {
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        *res.status_mut() = self.0;
        res
    }
}

impl<R> IntoResponse for (OverrideAllStatusCodes, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let (OverrideAllStatusCodes(status), res) = self;
        let mut res = res.into_response();
        *res.status_mut() = status;
        res
    }
}
