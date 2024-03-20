use crate::error_response::ErrorResponse;

use super::IntoResponse;

/// Trait for generating fallible responses in handlers.
///
/// This trait is bound by `IntoResponse` and therefor can be be interchanged with it
/// when returning a `Result` from a handler.
/// 
/// This trait is only implemented for `ResultResponse<T>` aka `Result<T, ErrorResponse>`
/// where both `T` and `ErrorResponse` implement `IntoResponse`.
///
/// The trait allows to return a `Result` from a handler.
pub trait IntoResultResponse: IntoResponse {}
impl<T: IntoResponse> IntoResultResponse for ResultResponse<T> {}

/// A type alias for `Result<T, ErrorResponse>`.
pub type ResultResponse<T, E = ErrorResponse> = std::result::Result<T, E>;
