use super::{IntoResponse, Response};
use crate::body::{boxed, Empty};
use http::{header::LOCATION, HeaderValue, StatusCode, Uri};
use std::convert::TryFrom;

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
///     .route("/old", get(|| async { Redirect::permanent("/new".parse().unwrap()) }))
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
    /// Create a new [`Redirect`] that uses a [`303 See Other`][mdn] status code.
    ///
    /// This redirect instructs the client to change the method to GET for the subsequent request
    /// to the given `uri`, which is useful after successful form submission, file upload or when
    /// you generally don't want the redirected-to page to observe the original request method and
    /// body (if non-empty). If you want to preserve the request method and body,
    /// [`Redirect::temporary`] should be used instead.
    ///
    /// # Panics
    ///
    /// If `uri` isn't a valid [`HeaderValue`].
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/303
    pub fn to(uri: Uri) -> Self {
        Self::with_status_code(StatusCode::SEE_OTHER, uri)
    }

    /// Create a new [`Redirect`] that uses a [`307 Temporary Redirect`][mdn] status code.
    ///
    /// This has the same behavior as [`Redirect::to`], except it will preserve the original HTTP
    /// method and body.
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
    /// This is the same as [`Redirect::temporary`] ([`307 Temporary Redirect`][mdn307]) except
    /// this status code is older and thus supported by some legacy clients that don't understand
    /// the newer one. Many clients wrongly apply [`Redirect::to`] ([`303 See Other`][mdn303])
    /// semantics for this status code, so it should be avoided where possible.
    ///
    /// # Panics
    ///
    /// If `uri` isn't a valid [`HeaderValue`].
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/302
    /// [mdn307]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/307
    /// [mdn303]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/303
    #[deprecated(
        note = "This results in different behavior between clients, so Redirect::temporary or Redirect::to should be used instead"
    )]
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
    fn into_response(self) -> Response {
        let mut res = Response::new(boxed(Empty::new()));
        *res.status_mut() = self.status_code;
        res.headers_mut().insert(LOCATION, self.location);
        res
    }
}
