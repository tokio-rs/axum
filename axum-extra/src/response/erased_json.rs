use axum::{
    body::{self, BoxBody, Full},
    http::{header, HeaderValue, Response, StatusCode},
    response::IntoResponse,
};
use serde::Serialize;

/// A response type that holds a JSON in serialized form.
///
/// This allows returning a borrowing type from a handler, or returning different response
/// types as JSON from different branches inside a handler.
///
/// # Example
///
/// ```rust
/// # use axum::{response::IntoResponse};
/// # use axum_extra::response::ErasedJson;
/// async fn handler() -> impl IntoResponse {
///     # let condition = true;
///     # let foo = ();
///     # let bar = vec![()];
///     // ...
///
///     if condition {
///         ErasedJson::new(&foo)
///     } else {
///         ErasedJson::new(&bar)
///     }
/// }
/// ```
#[derive(Debug)]
pub struct ErasedJson(serde_json::Result<Vec<u8>>);

impl ErasedJson {
    /// Create an `ErasedJson` by serializing a value.
    pub fn new<T: Serialize>(val: T) -> Self {
        Self(serde_json::to_vec(&val))
    }
}

impl IntoResponse for ErasedJson {
    fn into_response(self) -> Response<BoxBody> {
        let bytes = match self.0 {
            Ok(res) => res,
            Err(err) => {
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(header::CONTENT_TYPE, mime::TEXT_PLAIN_UTF_8.as_ref())
                    .body(body::boxed(Full::from(err.to_string())))
                    .unwrap();
            }
        };

        let mut res = Response::new(body::boxed(Full::from(bytes)));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
        );
        res
    }
}
