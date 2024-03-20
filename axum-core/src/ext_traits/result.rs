use std::error::Error;

use http::StatusCode;

use crate::{response::{IntoResponse, ResultResponse}, error_response::ErrorResponse};

/// A extention trait to Result to easily attach a `StatusCode` to an error by encapsulating the
///  error into a `ErrorResponse`.
pub trait ResultExt<T: IntoResponse> {
  /// maps the error type to a `ErrorResponse` with the given status code.
  fn err_with_status(self, status: StatusCode) -> ResultResponse<T>;
}

impl<T: IntoResponse, E: Into<Box<dyn Error>>> ResultExt<T> for std::result::Result<T, E> {
  fn err_with_status(self, status:StatusCode) -> ResultResponse<T> {
    self.map_err(|error| {
      ErrorResponse::new(status, error)
    })
  }
}
