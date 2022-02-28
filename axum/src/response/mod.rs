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

#[cfg(test)]
mod tests {
    use axum_core::response::IntoResponse;
    use http::Uri;

    use crate::{body::Body, routing::get, Router};

    #[test]
    fn impl_trait_result_works() {
        async fn impl_trait_ok() -> Result<impl IntoResponse, ()> {
            Ok(())
        }

        async fn impl_trait_err() -> Result<(), impl IntoResponse> {
            Err(())
        }

        async fn impl_trait_both(uri: Uri) -> Result<impl IntoResponse, impl IntoResponse> {
            if uri.path() == "/" {
                Ok(())
            } else {
                Err(())
            }
        }

        async fn impl_trait(uri: Uri) -> impl IntoResponse {
            if uri.path() == "/" {
                Ok(())
            } else {
                Err(())
            }
        }

        Router::<Body>::new()
            .route("/foo", get(impl_trait_ok))
            .route("/bar", get(impl_trait_err))
            .route("/baz", get(impl_trait_both))
            .route("/qux", get(impl_trait));
    }
}
