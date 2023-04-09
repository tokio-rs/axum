use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use http::header;
use http_body::Full;

/// A WASM response.
///
/// Will automatically get `Content-Type: application/wasm`.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct Wasm<T>(pub T);

impl<T> IntoResponse for Wasm<T>
where
    T: Into<Full<Bytes>>,
{
    fn into_response(self) -> Response {
        ([(header::CONTENT_TYPE, "application/wasm")], self.0.into()).into_response()
    }
}

impl<T> From<T> for Wasm<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}
