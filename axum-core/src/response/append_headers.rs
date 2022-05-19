use super::{IntoResponse, IntoResponseParts, Response, ResponseParts, TryIntoHeaderError};
use http::header::{HeaderName, HeaderValue};
use std::{convert::TryInto, fmt};

/// Append headers to a response.
///
/// Returning something like `[("content-type", "foo=bar")]` from a handler will override any
/// existing `content-type` headers. If instead you want to append headers, use `AppendHeaders`:
///
/// ```rust
/// use axum::{
///     response::{AppendHeaders, IntoResponse},
///     http::header::SET_COOKIE,
/// };
///
/// async fn handler() -> impl IntoResponse {
///     // something that sets the `set-cookie` header
///     let set_some_cookies = /* ... */
///     # axum::http::HeaderMap::new();
///
///     (
///         set_some_cookies,
///         // append two `set-cookie` headers to the response
///         // without overriding the ones added by `set_some_cookies`
///         AppendHeaders([
///             (SET_COOKIE, "foo=bar"),
///             (SET_COOKIE, "baz=qux"),
///         ])
///     )
/// }
/// ```
#[derive(Debug)]
pub struct AppendHeaders<K, V, const N: usize>(pub [(K, V); N]);

impl<K, V, const N: usize> IntoResponse for AppendHeaders<K, V, N>
where
    K: TryInto<HeaderName>,
    K::Error: fmt::Display,
    V: TryInto<HeaderValue>,
    V::Error: fmt::Display,
{
    fn into_response(self) -> Response {
        (self, ()).into_response()
    }
}

impl<K, V, const N: usize> IntoResponseParts for AppendHeaders<K, V, N>
where
    K: TryInto<HeaderName>,
    K::Error: fmt::Display,
    V: TryInto<HeaderValue>,
    V::Error: fmt::Display,
{
    type Error = TryIntoHeaderError<K::Error, V::Error>;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        for (key, value) in self.0 {
            let key = key.try_into().map_err(TryIntoHeaderError::key)?;
            let value = value.try_into().map_err(TryIntoHeaderError::value)?;
            res.headers_mut().append(key, value);
        }

        Ok(res)
    }
}
