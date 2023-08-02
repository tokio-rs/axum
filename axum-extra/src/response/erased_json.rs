use std::sync::Arc;

use axum::{
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
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
/// async fn handler() -> ErasedJson {
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
#[derive(Clone, Debug)]
#[must_use]
pub struct ErasedJson(Result<Bytes, Arc<serde_json::Error>>);

impl ErasedJson {
    /// Create an `ErasedJson` by serializing a value with the compact formatter.
    pub fn new<T: Serialize>(val: T) -> Self {
        let mut bytes = BytesMut::with_capacity(128);
        let result = match serde_json::to_writer((&mut bytes).writer(), &val) {
            Ok(()) => Ok(bytes.freeze()),
            Err(e) => Err(Arc::new(e)),
        };
        Self(result)
    }

    /// Create an `ErasedJson` by serializing a value with the pretty formatter.
    pub fn pretty<T: Serialize>(val: T) -> Self {
        let mut bytes = BytesMut::with_capacity(128);
        let result = match serde_json::to_writer_pretty((&mut bytes).writer(), &val) {
            Ok(()) => Ok(bytes.freeze()),
            Err(e) => Err(Arc::new(e)),
        };
        Self(result)
    }
}

impl IntoResponse for ErasedJson {
    fn into_response(self) -> Response {
        match self.0 {
            Ok(bytes) => (
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
                )],
                bytes,
            )
                .into_response(),
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
        }
    }
}
