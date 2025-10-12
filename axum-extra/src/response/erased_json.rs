use std::sync::Arc;

use axum_core::response::{IntoResponse, Response};
use bytes::{BufMut, Bytes, BytesMut};
use http::{header, HeaderValue, StatusCode};
use serde_core::Serialize;

/// A response type that holds a JSON in serialized form.
///
/// This allows returning a borrowing type from a handler, or returning different response
/// types as JSON from different branches inside a handler.
///
/// Like [`axum::Json`],
/// if the [`Serialize`] implementation fails
/// or if a map with non-string keys is used,
/// a 500 response will be issued
/// whose body is the error message in UTF-8.
///
/// This can be constructed using [`new`](ErasedJson::new)
/// or the [`json!`](crate::json) macro.
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
            Ok(()) => {
                bytes.put_u8(b'\n');
                Ok(bytes.freeze())
            }
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

/// Construct an [`ErasedJson`] response from a JSON literal.
///
/// A `Content-Type: application/json` header is automatically added.
/// Any variable or expression implementing [`Serialize`]
/// can be interpolated as a value in the literal.
/// If the [`Serialize`] implementation fails,
/// or if a map with non-string keys is used,
/// a 500 response will be issued
/// whose body is the error message in UTF-8.
///
/// Internally,
/// this function uses the [`typed_json::json!`] macro,
/// allowing it to perform far fewer allocations
/// than a dynamic macro like [`serde_json::json!`] would –
/// it's equivalent to if you had just written
/// `derive(Serialize)` on a struct.
///
/// # Examples
///
/// ```
/// use axum::{
///     Router,
///     extract::Path,
///     response::Response,
///     routing::get,
/// };
/// use axum_extra::response::ErasedJson;
///
/// async fn get_user(Path(user_id) : Path<u64>) -> ErasedJson {
///     let user_name = find_user_name(user_id).await;
///     axum_extra::json!({ "name": user_name })
/// }
///
/// async fn find_user_name(user_id: u64) -> String {
///     // ...
///     # unimplemented!()
/// }
///
/// let app = Router::new().route("/users/{id}", get(get_user));
/// # let _: Router = app;
/// ```
///
/// Trailing commas are allowed in both arrays and objects.
///
/// ```
/// let response = axum_extra::json!(["trailing",]);
/// ```
#[macro_export]
macro_rules! json {
    ($($t:tt)*) => {
        $crate::response::ErasedJson::new(
            $crate::response::__private_erased_json::typed_json::json!($($t)*)
        )
    }
}

/// Not public API. Re-exported as `crate::response::__private_erased_json`.
#[doc(hidden)]
pub mod private {
    pub use typed_json;
}
