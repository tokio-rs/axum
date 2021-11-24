#![doc = include_str!("../docs/response.md")]

use bytes::Bytes;
use http::{header, HeaderValue, Response};
use http_body::Full;
use std::convert::Infallible;

mod redirect;

pub mod sse;

#[doc(no_inline)]
#[cfg(feature = "json")]
pub use crate::Json;

#[doc(inline)]
pub use axum_core::response::{Headers, IntoResponse};

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
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        #[allow(clippy::declare_interior_mutable_const)]
        const TEXT_HTML: HeaderValue = HeaderValue::from_static("text/html");

        let mut res = Response::new(self.0.into());
        res.headers_mut().insert(header::CONTENT_TYPE, TEXT_HTML);
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
    use crate::body::Body;
    use http::{
        header::{HeaderMap, HeaderName},
        StatusCode,
    };

    #[test]
    fn test_merge_headers() {
        struct MyResponse;

        impl IntoResponse for MyResponse {
            type Body = Body;
            type BodyError = <Self::Body as http_body::Body>::Error;

            fn into_response(self) -> Response<Body> {
                let mut resp = Response::new(String::new().into());
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
