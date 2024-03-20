use std::error::Error;

use http::StatusCode;

use crate::response::{IntoResponse, Response};

/// `ErrorResponse` encapsulates an `Error` and a `StatusCode` to be used as a response, typically in a `Result`.
/// 
/// If not `StatusCode` is provided, `StatusCode::INTERNAL_SERVER_ERROR` is used.
#[derive(Debug)]
pub struct ErrorResponse {
  status: StatusCode,
  error: Box<dyn Error>,
}

impl ErrorResponse {
  /// Create a new `ErrorResponse` with the given status code and error.
  pub fn new(status: StatusCode, error: impl Into<Box<dyn Error>>) -> Self {
    Self {
      status,
      error: error.into(),
    }
  }
}

impl<E: Into<Box<dyn Error>>> From<E> for ErrorResponse
{
  fn from(error: E) -> Self {
    Self::new(StatusCode::INTERNAL_SERVER_ERROR, error)
  }
}

impl IntoResponse for ErrorResponse {
  fn into_response(self) -> Response {
    let error = format!("{:?}", self.error);

    #[cfg(feature = "tracing")]
    tracing::error!(error = %error);

    (self.status, error).into_response()
  }
}
