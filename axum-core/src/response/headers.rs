use super::{IntoResponse, IntoResponseHeaders, Response};
use http::{
    header::{HeaderName, HeaderValue},
    StatusCode,
};
use std::{convert::TryInto, fmt};

/// A response with headers.
///
/// # Example
///
/// ```rust
/// use axum::{
///     Router,
///     response::{IntoResponse, Headers},
///     routing::get,
/// };
/// use http::header::{HeaderName, HeaderValue};
///
/// // It works with any `IntoIterator<Item = (Key, Value)>` where `Key` can be
/// // turned into a `HeaderName` and `Value` can be turned into a `HeaderValue`
/// //
/// // Such as `Vec<(HeaderName, HeaderValue)>`
/// async fn just_headers() -> impl IntoResponse {
///     Headers(vec![
///         (HeaderName::from_static("X-Foo"), HeaderValue::from_static("foo")),
///     ])
/// }
///
/// // Or `Vec<(&str, &str)>`
/// async fn from_strings() -> impl IntoResponse {
///     Headers(vec![("X-Foo", "foo")])
/// }
///
/// // Or `[(&str, &str)]` if you're on Rust 1.53+
///
/// let app = Router::new()
///     .route("/just-headers", get(just_headers))
///     .route("/from-strings", get(from_strings));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// If a conversion to `HeaderName` or `HeaderValue` fails a `500 Internal
/// Server Error` response will be returned.
///
/// You can also return `(Headers, impl IntoResponse)` to customize the headers
/// of a response, or `(StatusCode, Headers, impl IntoResponse)` to customize
/// the status code and headers.
#[derive(Clone, Copy, Debug)]
pub struct Headers<H>(pub H);

impl<H, K, V> IntoResponseHeaders for Headers<H>
where
    H: IntoIterator<Item = (K, V)>,
    K: TryInto<HeaderName>,
    K::Error: fmt::Display,
    V: TryInto<HeaderValue>,
    V::Error: fmt::Display,
{
    type IntoIter = IntoIter<H::IntoIter>;

    fn into_headers(self) -> Self::IntoIter {
        IntoIter {
            inner: self.0.into_iter(),
        }
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct IntoIter<H> {
    inner: H,
}

impl<H, K, V> Iterator for IntoIter<H>
where
    H: Iterator<Item = (K, V)>,
    K: TryInto<HeaderName>,
    K::Error: fmt::Display,
    V: TryInto<HeaderValue>,
    V::Error: fmt::Display,
{
    type Item = Result<(Option<HeaderName>, HeaderValue), Response>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(key, value)| {
            let key = key
                .try_into()
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?;
            let value = value
                .try_into()
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?;

            Ok((Some(key), value))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::FutureExt;
    use http::header::USER_AGENT;

    #[test]
    fn vec_of_header_name_and_value() {
        let res = Headers(vec![(USER_AGENT, HeaderValue::from_static("axum"))]).into_response();

        assert_eq!(res.headers()["user-agent"], "axum");
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[test]
    fn vec_of_strings() {
        let res = Headers(vec![("user-agent", "axum")]).into_response();

        assert_eq!(res.headers()["user-agent"], "axum");
    }

    #[test]
    fn with_body() {
        let res = (Headers(vec![("user-agent", "axum")]), "foo").into_response();

        assert_eq!(res.headers()["user-agent"], "axum");
        let body = crate::body::to_bytes(res.into_body())
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(&body[..], b"foo");
    }

    #[test]
    fn with_status_and_body() {
        let res = (
            StatusCode::NOT_FOUND,
            Headers(vec![("user-agent", "axum")]),
            "foo",
        )
            .into_response();

        assert_eq!(res.headers()["user-agent"], "axum");
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let body = crate::body::to_bytes(res.into_body())
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(&body[..], b"foo");
    }

    #[test]
    fn invalid_header_name() {
        let bytes: &[u8] = &[0, 159, 146, 150]; // invalid utf-8
        let res = Headers(vec![(bytes, "axum")]).into_response();

        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn invalid_header_value() {
        let bytes: &[u8] = &[0, 159, 146, 150]; // invalid utf-8
        let res = Headers(vec![("user-agent", bytes)]).into_response();

        assert!(res.headers().get("user-agent").is_none());
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
