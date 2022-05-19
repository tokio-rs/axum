#![doc = include_str!("../docs/response.md")]

use crate::body::{Bytes, Full};
use http::{header, HeaderValue};

mod redirect;

pub mod sse;

#[doc(no_inline)]
#[cfg(feature = "json")]
pub use crate::Json;

#[doc(no_inline)]
#[cfg(feature = "headers")]
pub use crate::TypedHeader;

#[doc(no_inline)]
pub use crate::Extension;

#[doc(inline)]
pub use axum_core::response::{
    AppendHeaders, ErrorResponse, IntoResponse, IntoResponseParts, Response, ResponseParts, Result,
};

#[doc(inline)]
pub use self::{redirect::Redirect, sse::Sse};

/// An HTML response.
///
/// Will automatically get `Content-Type: text/html`.
#[derive(Clone, Copy, Debug)]
pub struct Html<T>(pub T);

impl<T> IntoResponse for Html<T>
where
    T: Into<Full<Bytes>>,
{
    fn into_response(self) -> Response {
        (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static(mime::TEXT_HTML_UTF_8.as_ref()),
            )],
            self.0.into(),
        )
            .into_response()
    }
}

impl<T> From<T> for Html<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

#[cfg(test)]
mod tests {
    use crate::extract::Extension;
    use crate::{body::Body, routing::get, Router};
    use axum_core::response::IntoResponse;
    use http::HeaderMap;
    use http::{StatusCode, Uri};

    // just needs to compile
    #[allow(dead_code)]
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
            .route("/", get(impl_trait_ok))
            .route("/", get(impl_trait_err))
            .route("/", get(impl_trait_both))
            .route("/", get(impl_trait));
    }

    // just needs to compile
    #[allow(dead_code)]
    fn tuple_responses() {
        async fn status() -> impl IntoResponse {
            StatusCode::OK
        }

        async fn status_headermap() -> impl IntoResponse {
            (StatusCode::OK, HeaderMap::new())
        }

        async fn status_header_array() -> impl IntoResponse {
            (StatusCode::OK, [("content-type", "text/plain")])
        }

        async fn status_headermap_body() -> impl IntoResponse {
            (StatusCode::OK, HeaderMap::new(), String::new())
        }

        async fn status_header_array_body() -> impl IntoResponse {
            (
                StatusCode::OK,
                [("content-type", "text/plain")],
                String::new(),
            )
        }

        async fn status_headermap_impl_into_response() -> impl IntoResponse {
            (StatusCode::OK, HeaderMap::new(), impl_into_response())
        }

        async fn status_header_array_impl_into_response() -> impl IntoResponse {
            (
                StatusCode::OK,
                [("content-type", "text/plain")],
                impl_into_response(),
            )
        }

        fn impl_into_response() -> impl IntoResponse {}

        async fn status_header_array_extension_body() -> impl IntoResponse {
            (
                StatusCode::OK,
                [("content-type", "text/plain")],
                Extension(1),
                String::new(),
            )
        }

        async fn status_header_array_extension_mixed_body() -> impl IntoResponse {
            (
                StatusCode::OK,
                [("content-type", "text/plain")],
                Extension(1),
                HeaderMap::new(),
                String::new(),
            )
        }

        //

        async fn headermap() -> impl IntoResponse {
            HeaderMap::new()
        }

        async fn header_array() -> impl IntoResponse {
            [("content-type", "text/plain")]
        }

        async fn headermap_body() -> impl IntoResponse {
            (HeaderMap::new(), String::new())
        }

        async fn header_array_body() -> impl IntoResponse {
            ([("content-type", "text/plain")], String::new())
        }

        async fn headermap_impl_into_response() -> impl IntoResponse {
            (HeaderMap::new(), impl_into_response())
        }

        async fn header_array_impl_into_response() -> impl IntoResponse {
            ([("content-type", "text/plain")], impl_into_response())
        }

        async fn header_array_extension_body() -> impl IntoResponse {
            (
                [("content-type", "text/plain")],
                Extension(1),
                String::new(),
            )
        }

        async fn header_array_extension_mixed_body() -> impl IntoResponse {
            (
                [("content-type", "text/plain")],
                Extension(1),
                HeaderMap::new(),
                String::new(),
            )
        }

        Router::<Body>::new()
            .route("/", get(status))
            .route("/", get(status_headermap))
            .route("/", get(status_header_array))
            .route("/", get(status_headermap_body))
            .route("/", get(status_header_array_body))
            .route("/", get(status_headermap_impl_into_response))
            .route("/", get(status_header_array_impl_into_response))
            .route("/", get(status_header_array_extension_body))
            .route("/", get(status_header_array_extension_mixed_body))
            .route("/", get(headermap))
            .route("/", get(header_array))
            .route("/", get(headermap_body))
            .route("/", get(header_array_body))
            .route("/", get(headermap_impl_into_response))
            .route("/", get(header_array_impl_into_response))
            .route("/", get(header_array_extension_body))
            .route("/", get(header_array_extension_mixed_body));
    }
}
