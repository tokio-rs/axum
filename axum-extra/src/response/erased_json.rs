use axum::{
    body::Full,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponseParts, ResponseParts},
};
use bytes::{BufMut, Bytes, BytesMut};
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
#[cfg_attr(docsrs, doc(cfg(feature = "erased-json")))]
#[derive(Debug)]
pub struct ErasedJson(serde_json::Result<Bytes>);

impl ErasedJson {
    /// Create an `ErasedJson` by serializing a value with the compact formatter.
    pub fn new<T: Serialize>(val: T) -> Self {
        let mut bytes = BytesMut::with_capacity(128);
        Self(serde_json::to_writer((&mut bytes).writer(), &val).map(|_| bytes.freeze()))
    }

    /// Create an `ErasedJson` by serializing a value with the pretty formatter.
    pub fn pretty<T: Serialize>(val: T) -> Self {
        let mut bytes = BytesMut::with_capacity(128);
        Self(serde_json::to_writer_pretty((&mut bytes).writer(), &val).map(|_| bytes.freeze()))
    }
}

impl IntoResponseParts for ErasedJson {
    fn into_response_parts(self, res: &mut ResponseParts) {
        match self.0 {
            Ok(bytes) => {
                res.set_body(Full::from(bytes));
                res.insert_header(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
                );
            }
            Err(err) => {
                res.set_status(StatusCode::INTERNAL_SERVER_ERROR);
                res.insert_header(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
                );
                res.set_body(Full::from(err.to_string()));
            }
        }
    }
}
