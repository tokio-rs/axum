#![doc = include_str!("../docs/response.md")]

use crate::body::{Bytes, Full};
use http::{header, HeaderValue};

mod redirect;

pub mod sse;

#[doc(no_inline)]
#[cfg(feature = "json")]
pub use crate::Json;

#[doc(inline)]
pub use axum_core::response::{IntoResponse, IntoResponseParts, Response, ResponseParts};

#[doc(inline)]
pub use self::{redirect::Redirect, sse::Sse};

/// An HTML response.
///
/// Will automatically get `Content-Type: text/html`.
#[derive(Clone, Copy, Debug)]
pub struct Html<T>(pub T);

impl<T> IntoResponseParts for Html<T>
where
    T: Into<Full<Bytes>>,
{
    fn into_response_parts(self, res: &mut ResponseParts) {
        res.set_body(self.0.into());
        res.insert_header(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::TEXT_HTML_UTF_8.as_ref()),
        );
    }
}

impl<T> From<T> for Html<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}
