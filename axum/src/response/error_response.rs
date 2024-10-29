use axum_core::response::{IntoResponse, Response};
use http::StatusCode;
use std::error::Error;
use std::fmt::Display;
use std::io::Write;

/// Convenience response to create an error response from a non-IntoResponse error
///
/// This provides a method to quickly respond with an error that does not implement
/// the IntoResponse trait itself. This type should only be used for debugging purposes or internal
/// facing applications, as it includes the full error chain with descriptions,
/// thus leaking information that could possibly be sensitive.
///
/// ```rust,no_run
/// use axum::response::{InternalServerError, IntoResponse, NoContent};
/// # use std::io::{Error, ErrorKind};
/// # fn try_thing() -> Result<(), Error> {
/// #   Err(Error::new(ErrorKind::Other, "error"))
/// # }
///
/// async fn maybe_error() -> Result<String, InternalServerError<Error>> {
///     try_thing().map_err(InternalServerError)?;
///     // do something on success
///     # Ok(String::from("ok"))
/// }
/// ```
#[derive(Debug)]
pub struct InternalServerError<T>(pub T);

impl<T: Error> IntoResponse for InternalServerError<T> {
    fn into_response(self) -> Response {
        let mut body = Vec::new();
        write!(body, "{}", self.0);
        let mut e: &dyn Error = &self.0;
        while let Some(new_e) = e.source() {
            e = new_e;
            write!(body, ": {e}").unwrap();
        }
        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error, ErrorKind};

    #[test]
    fn internal_server_error() {
        let response = InternalServerError(Error::new(ErrorKind::Other, "Test")).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn with_error_chain() {
        todo!();
    }

}
