#![doc = include_str!("../docs/response.md")]

use crate::body::{Bytes, Full};
use axum_core::body::boxed;
use http::{header, HeaderValue};

mod redirect;

pub mod sse;

#[doc(no_inline)]
#[cfg(feature = "json")]
pub use crate::Json;

#[doc(inline)]
pub use axum_core::response::{Headers, IntoResponse, Response};

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
        let mut res = Response::new(boxed(self.0.into()));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::TEXT_HTML_UTF_8.as_ref()),
        );
        res
    }
}

impl<T> From<T> for Html<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::Empty;
    use http::{
        header::{HeaderMap, HeaderName},
        StatusCode,
    };

    #[test]
    fn test_merge_headers() {
        struct MyResponse;

        impl IntoResponse for MyResponse {
            fn into_response(self) -> Response {
                let mut resp = Response::new(boxed(Empty::new()));
                resp.headers_mut()
                    .insert(HeaderName::from_static("a"), HeaderValue::from_static("1"));
                resp
            }
        }

        fn check(resp: impl IntoResponse) {
            let resp = resp.into_response();
            assert_eq!(
                resp.headers().get(HeaderName::from_static("a")).unwrap(),
                &HeaderValue::from_static("1")
            );
            assert_eq!(
                resp.headers().get(HeaderName::from_static("b")).unwrap(),
                &HeaderValue::from_static("2")
            );
        }

        let headers: HeaderMap =
            std::iter::once((HeaderName::from_static("b"), HeaderValue::from_static("2")))
                .collect();

        check((headers.clone(), MyResponse));
        check((StatusCode::OK, headers, MyResponse));
    }
}
