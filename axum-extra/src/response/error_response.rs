use axum_core::response::{IntoResponse, Response};
use http::StatusCode;
use std::error::Error;
use tracing::error;

/// Convenience response to create an error response from a non-[`IntoResponse`] error
///
/// This provides a method to quickly respond with an error that does not implement
/// the `IntoResponse` trait itself. This type should only be used for debugging purposes or internal
/// facing applications, as it includes the full error chain with descriptions,
/// thus leaking information that could possibly be sensitive.
///
/// ```rust
/// use axum_extra::response::InternalServerError;
/// use axum_core::response::IntoResponse;
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

impl<T: Error + 'static> IntoResponse for InternalServerError<T> {
    fn into_response(self) -> Response {
        error!(error = &self.0 as &dyn Error);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "An error occurred while processing your request.",
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Error;

    #[test]
    fn internal_server_error() {
        let response = InternalServerError(Error::other("Test")).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
