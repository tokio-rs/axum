//! Types and traits for generating responses.
//!
//! See [`axum::response`] for more details.
//!
//! [`axum::response`]: https://docs.rs/axum/0.8/axum/response/index.html

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
#[must_use]
pub struct ErrorResponse(Response);

impl<T> From<T> for ErrorResponse
where
    T: IntoResponse,
{
    fn from(value: T) -> Self {
        Self(value.into_response())
    }
}

/// Response part that stops status code overrides.
///
/// This type should be used by types implementing [`IntoResponseParts`] or
/// [`IntoResponse`] when they fail to produce the response usually expected of
/// them and return some sort of error response instead.
///
/// It is checked used by the tuple impls of [`IntoResponse`] that have a
/// [`StatusCode`] as their first element to ignore that status code.
/// Consider the following example:
///
/// ```no_run
/// # use axum::Json;
/// # use http::StatusCode;
/// # #[derive(serde::Serialize)]
/// # struct CreatedResponse { }
/// fn my_handler(/* ... */) -> (StatusCode, Json<CreatedResponse>) {
///     // This response type's serialization may fail
///     let response = CreatedResponse { /* ... */ };
///     (StatusCode::CREATED, Json(response))
/// }
/// ```
///
/// When `response` serialization succeeds, the server responds with a status
/// code of 201 Created (overwriting `Json`s default status code of 200 OK),
/// and the expected JSON payload.
///
/// When `response` serialization fails hoewever, `impl IntoResponse for Json`
/// return a response with status code 500 Internal Server Error, and
/// `IntoResponseFailed` as a response extension, and the 201 Created override
/// is ignored.
///
/// This is a behavior introduced with axum 0.9.\
/// To force a status code override even when an inner [`IntoResponseParts`] /
/// [`IntoResponse`] failed, use [`ForceStatusCode`].
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

/// Set the status code regardless of whether [`IntoResponseFailed`] is used or not.
///
/// See the docs for [`IntoResponseFailed`] for more details.
#[derive(Debug, Copy, Clone, Default)]
pub struct ForceStatusCode(pub StatusCode);

impl IntoResponse for ForceStatusCode {
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        *res.status_mut() = self.0;
        res
    }
}

impl<R> IntoResponse for (ForceStatusCode, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let (ForceStatusCode(status), res) = self;
        let mut res = res.into_response();
        *res.status_mut() = status;
        res
    }
}
