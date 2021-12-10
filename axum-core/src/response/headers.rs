use super::{IntoResponse, Response};
use crate::body::boxed;
use bytes::Bytes;
use http::{
    header::{HeaderMap, HeaderName, HeaderValue},
    StatusCode,
};
use http_body::{Empty, Full};
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

impl<H> Headers<H> {
    fn try_into_header_map<K, V>(self) -> Result<HeaderMap, Response>
    where
        H: IntoIterator<Item = (K, V)>,
        K: TryInto<HeaderName>,
        K::Error: fmt::Display,
        V: TryInto<HeaderValue>,
        V::Error: fmt::Display,
    {
        self.0
            .into_iter()
            .map(|(key, value)| {
                let key = key.try_into().map_err(Either::A)?;
                let value = value.try_into().map_err(Either::B)?;
                Ok((key, value))
            })
            .collect::<Result<_, _>>()
            .map_err(|err| {
                let err = match err {
                    Either::A(err) => err.to_string(),
                    Either::B(err) => err.to_string(),
                };

                let body = boxed(Full::new(Bytes::copy_from_slice(err.as_bytes())));
                let mut res = Response::new(body);
                *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                res
            })
    }
}

impl<H, K, V> IntoResponse for Headers<H>
where
    H: IntoIterator<Item = (K, V)>,
    K: TryInto<HeaderName>,
    K::Error: fmt::Display,
    V: TryInto<HeaderValue>,
    V::Error: fmt::Display,
{
    fn into_response(self) -> Response {
        let headers = self.try_into_header_map();

        match headers {
            Ok(headers) => {
                let mut res = Response::new(boxed(Empty::new()));
                *res.headers_mut() = headers;
                res
            }
            Err(err) => err,
        }
    }
}

impl<H, T, K, V> IntoResponse for (Headers<H>, T)
where
    T: IntoResponse,
    H: IntoIterator<Item = (K, V)>,
    K: TryInto<HeaderName>,
    K::Error: fmt::Display,
    V: TryInto<HeaderValue>,
    V::Error: fmt::Display,
{
    fn into_response(self) -> Response {
        let headers = match self.0.try_into_header_map() {
            Ok(headers) => headers,
            Err(res) => return res,
        };

        (headers, self.1).into_response()
    }
}

impl<H, T, K, V> IntoResponse for (StatusCode, Headers<H>, T)
where
    T: IntoResponse,
    H: IntoIterator<Item = (K, V)>,
    K: TryInto<HeaderName>,
    K::Error: fmt::Display,
    V: TryInto<HeaderValue>,
    V::Error: fmt::Display,
{
    fn into_response(self) -> Response {
        let headers = match self.1.try_into_header_map() {
            Ok(headers) => headers,
            Err(res) => return res,
        };

        (self.0, headers, self.2).into_response()
    }
}

enum Either<A, B> {
    A(A),
    B(B),
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
