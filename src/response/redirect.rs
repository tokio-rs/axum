use super::IntoResponse;
use bytes::Bytes;
use http::{header::LOCATION, HeaderValue, Response, StatusCode, Uri};
use http_body::{Body, Empty};
use std::convert::TryFrom;

/// Response that redirects the request to another location.
///
/// # Example
///
/// ```rust
/// use axum::{
///     handler::get,
///     response::Redirect,
///     route,
/// };
///
/// let app = route("/old", get(|| async { Redirect::permanent("/new".parse().unwrap()) }))
///     .route("/new", get(|| async { "Hello!" }));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
#[derive(Debug, Clone)]
pub struct Redirect {
    status_code: StatusCode,
    location: HeaderValue,
}

impl Redirect {
    /// Create a new [`Redirect`] that uses a [`307 Temporary Redirect`][mdn] status code.
    ///
    /// # Panics
    ///
    /// If `uri` isn't a valid [`HeaderValue`].
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/307
    pub fn temporary(uri: Uri) -> Self {
        Self::with_status_code(StatusCode::TEMPORARY_REDIRECT, uri)
    }

    /// Create a new [`Redirect`] that uses a [`308 Permanent Redirect`][mdn] status code.
    ///
    /// # Panics
    ///
    /// If `uri` isn't a valid [`HeaderValue`].
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/308
    pub fn permanent(uri: Uri) -> Self {
        Self::with_status_code(StatusCode::PERMANENT_REDIRECT, uri)
    }

    /// Create a new [`Redirect`] that uses a [`302 Found`][mdn] status code.
    ///
    /// # Panics
    ///
    /// If `uri` isn't a valid [`HeaderValue`].
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/302
    pub fn found(uri: Uri) -> Self {
        Self::with_status_code(StatusCode::FOUND, uri)
    }

    // This is intentionally not public since other kinds of redirects might not
    // use the `Location` header, namely `304 Not Modified`.
    //
    // We're open to adding more constructors upon request, if they make sense :)
    fn with_status_code(status_code: StatusCode, uri: Uri) -> Self {
        assert!(
            status_code.is_redirection(),
            "not a redirection status code"
        );

        Self {
            status_code,
            location: HeaderValue::try_from(uri.to_string())
                .expect("URI isn't a valid header value"),
        }
    }
}

impl IntoResponse for Redirect {
    type Body = Empty<Bytes>;
    type BodyError = <Self::Body as Body>::Error;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Empty::new());
        *res.status_mut() = self.status_code;
        res.headers_mut().insert(LOCATION, self.location);
        res
    }
}
