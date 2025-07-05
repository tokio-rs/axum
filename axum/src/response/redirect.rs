use axum_core::response::{IntoResponse, Response};
use http::{header::LOCATION, HeaderValue, StatusCode};

/// Response that redirects the request to another location.
///
/// # Example
///
/// ```rust
/// use axum::{
///     routing::get,
///     response::Redirect,
///     Router,
/// };
///
/// let app = Router::new()
///     .route("/old", get(|| async { Redirect::permanent("/new") }))
///     .route("/new", get(|| async { "Hello!" }));
/// # let _: Router = app;
/// ```
#[must_use = "needs to be returned from a handler or otherwise turned into a Response to be useful"]
#[derive(Debug, Clone)]
pub struct Redirect {
    status_code: StatusCode,
    location: String,
}

impl Redirect {
    /// Create a new [`Redirect`] that uses a [`303 See Other`][mdn] status code.
    ///
    /// This redirect instructs the client to change the method to GET for the subsequent request
    /// to the given `uri`, which is useful after successful form submission, file upload or when
    /// you generally don't want the redirected-to page to observe the original request method and
    /// body (if non-empty). If you want to preserve the request method and body,
    /// [`Redirect::temporary`] should be used instead.
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/303
    pub fn to(uri: &str) -> Self {
        Self::with_status_code(StatusCode::SEE_OTHER, uri)
    }

    /// Create a new [`Redirect`] that uses a [`307 Temporary Redirect`][mdn] status code.
    ///
    /// This has the same behavior as [`Redirect::to`], except it will preserve the original HTTP
    /// method and body.
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/307
    pub fn temporary(uri: &str) -> Self {
        Self::with_status_code(StatusCode::TEMPORARY_REDIRECT, uri)
    }

    /// Create a new [`Redirect`] that uses a [`308 Permanent Redirect`][mdn] status code.
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/308
    pub fn permanent(uri: &str) -> Self {
        Self::with_status_code(StatusCode::PERMANENT_REDIRECT, uri)
    }

    /// Returns the HTTP status code of the `Redirect`.
    #[must_use]
    pub fn status_code(&self) -> StatusCode {
        self.status_code
    }

    /// Returns the `Redirect`'s URI.
    #[must_use]
    pub fn location(&self) -> &str {
        &self.location
    }

    // This is intentionally not public since other kinds of redirects might not
    // use the `Location` header, namely `304 Not Modified`.
    //
    // We're open to adding more constructors upon request, if they make sense :)
    fn with_status_code(status_code: StatusCode, uri: &str) -> Self {
        assert!(
            status_code.is_redirection(),
            "not a redirection status code"
        );

        Self {
            status_code,
            location: uri.to_owned(),
        }
    }
}

impl IntoResponse for Redirect {
    fn into_response(self) -> Response {
        match HeaderValue::try_from(self.location) {
            Ok(location) => (self.status_code, [(LOCATION, location)]).into_response(),
            Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Redirect;
    use axum_core::response::IntoResponse;
    use http::StatusCode;

    const EXAMPLE_URL: &str = "https://example.com";

    // Tests to make sure Redirect has the correct status codes
    // based on the way it was constructed.
    #[test]
    fn correct_status() {
        assert_eq!(
            StatusCode::SEE_OTHER,
            Redirect::to(EXAMPLE_URL).status_code()
        );

        assert_eq!(
            StatusCode::TEMPORARY_REDIRECT,
            Redirect::temporary(EXAMPLE_URL).status_code()
        );

        assert_eq!(
            StatusCode::PERMANENT_REDIRECT,
            Redirect::permanent(EXAMPLE_URL).status_code()
        );
    }

    #[test]
    fn correct_location() {
        assert_eq!(EXAMPLE_URL, Redirect::permanent(EXAMPLE_URL).location());

        assert_eq!("/redirect", Redirect::permanent("/redirect").location())
    }

    #[test]
    fn test_internal_error() {
        let response = Redirect::permanent("Axum is awesome, \n but newlines aren't allowed :(")
            .into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
