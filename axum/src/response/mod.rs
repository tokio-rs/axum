#![doc = include_str!("../docs/response.md")]

use axum_core::body::Body;
use http::{header, HeaderValue};

mod redirect;

#[cfg(feature = "tokio")]
pub mod sse;

#[doc(no_inline)]
#[cfg(feature = "json")]
pub use crate::Json;

#[cfg(feature = "form")]
#[doc(no_inline)]
pub use crate::form::Form;

#[doc(no_inline)]
pub use crate::Extension;

#[doc(inline)]
pub use axum_core::response::{
    AppendHeaders, ErrorResponse, IntoResponse, IntoResponseParts, Response, ResponseParts, Result,
};

#[doc(inline)]
pub use self::redirect::Redirect;

#[doc(inline)]
#[cfg(feature = "tokio")]
pub use sse::Sse;

/// An HTML response.
///
/// Will automatically get `Content-Type: text/html`.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct Html<T>(pub T);

impl<T> IntoResponse for Html<T>
where
    T: Into<Body>,
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
    use crate::test_helpers::*;
    use crate::Json;
    use crate::{routing::get, Router};
    use axum_core::response::{
        IntoResponse, IntoResponseFailed, IntoResponseParts, Response, ResponseParts,
    };
    use http::HeaderMap;
    use http::{StatusCode, Uri};
    use std::collections::HashMap;

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

        _ = Router::<()>::new()
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

        _ = Router::<()>::new()
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

    #[test]
    fn status_code_tuple_doesnt_override_error() {
        // sanity check where there is just one status code
        assert_eq!(
            StatusCode::INTERNAL_SERVER_ERROR.into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            (StatusCode::INTERNAL_SERVER_ERROR,)
                .into_response()
                .status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );

        // non-5xx status should be changed
        assert_eq!(
            (StatusCode::SEE_OTHER, StatusCode::NO_CONTENT)
                .into_response()
                .status(),
            StatusCode::SEE_OTHER
        );
        let res = (
            StatusCode::SEE_OTHER,
            [("location", "foo")],
            StatusCode::NO_CONTENT,
        )
            .into_response();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        assert_eq!(res.headers()["location"], "foo");

        // 5xx status codes are also changed
        assert_eq!(
            (StatusCode::SEE_OTHER, StatusCode::INTERNAL_SERVER_ERROR)
                .into_response()
                .status(),
            StatusCode::SEE_OTHER
        );
        let res = (
            StatusCode::SEE_OTHER,
            [("location", "foo")],
            StatusCode::INTERNAL_SERVER_ERROR,
        )
            .into_response();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        assert_eq!(res.headers()["location"], "foo");

        // the status is not changed if `IntoResponseFailed` is used
        assert_eq!(
            (
                StatusCode::SEE_OTHER,
                (IntoResponseFailed, StatusCode::INTERNAL_SERVER_ERROR)
            )
                .into_response()
                .status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        let res = (
            StatusCode::SEE_OTHER,
            [("location", "foo")],
            (IntoResponseFailed, StatusCode::INTERNAL_SERVER_ERROR),
        )
            .into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(res.headers().get("location").is_none());

        // response parts from the inner response do run
        let res = (
            // with status override
            StatusCode::SEE_OTHER,
            [("location", "foo")],
            (
                [("x-bar", "bar")],
                IntoResponseFailed,
                [("x-foo", "foo")],
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        )
            .into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(res.headers().get("location").is_none());
        assert_eq!(res.headers()["x-foo"], "foo");
        assert_eq!(res.headers()["x-bar"], "bar");

        let res = (
            // without status override
            [("location", "foo")],
            (
                [("x-bar", "bar")],
                IntoResponseFailed,
                [("x-foo", "foo")],
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        )
            .into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(res.headers().get("location").is_none());
        assert_eq!(res.headers()["x-foo"], "foo");
        assert_eq!(res.headers()["x-bar"], "bar");

        // (Parts, ...)
        let res = (
            Response::new(()).into_parts().0,
            [("location", "foo")],
            (
                [("x-bar", "bar")],
                IntoResponseFailed,
                [("x-foo", "foo")],
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        )
            .into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(res.headers().get("location").is_none());
        assert_eq!(res.headers()["x-foo"], "foo");
        assert_eq!(res.headers()["x-bar"], "bar");

        // (Response<()>, ...)
        let res = (
            Response::new(()),
            [("location", "foo")],
            (
                [("x-bar", "bar")],
                IntoResponseFailed,
                [("x-foo", "foo")],
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        )
            .into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(res.headers().get("location").is_none());
        assert_eq!(res.headers()["x-foo"], "foo");
        assert_eq!(res.headers()["x-bar"], "bar");
    }

    #[test]
    fn into_response_parts_failing_sets_extension() {
        struct Fail;

        impl IntoResponseParts for Fail {
            type Error = ();

            fn into_response_parts(
                self,
                _res: ResponseParts,
            ) -> Result<ResponseParts, Self::Error> {
                Err(())
            }
        }

        impl IntoResponse for Fail {
            fn into_response(self) -> Response {
                (self, ()).into_response()
            }
        }

        assert!(Fail
            .into_response()
            .extensions()
            .get::<IntoResponseFailed>()
            .is_some());

        assert!((StatusCode::INTERNAL_SERVER_ERROR, Fail, ())
            .into_response()
            .extensions()
            .get::<IntoResponseFailed>()
            .is_some());

        assert!((Response::new(()).into_parts().0, Fail, ())
            .into_response()
            .extensions()
            .get::<IntoResponseFailed>()
            .is_some());

        assert!((Response::new(()), Fail, ())
            .into_response()
            .extensions()
            .get::<IntoResponseFailed>()
            .is_some());
    }

    #[test]
    fn doenst_override_status_code_when_using_into_response_failed_at_same_level() {
        assert_eq!(
            (StatusCode::INTERNAL_SERVER_ERROR, IntoResponseFailed, ())
                .into_response()
                .status(),
            StatusCode::INTERNAL_SERVER_ERROR,
        );

        #[derive(Clone)]
        struct Thing;

        let res = (
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("x-foo", "foo")
                .extension(Thing)
                .body(())
                .unwrap()
                .into_parts()
                .0,
            IntoResponseFailed,
            (),
        )
            .into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR,);
        assert_eq!(res.headers()["x-foo"], "foo");
        assert!(res.extensions().get::<Thing>().is_some());

        // just a sanity check
        assert_eq!(
            (IntoResponseFailed, ()).into_response().status(),
            StatusCode::OK,
        );
    }

    #[crate::test]
    async fn status_code_tuple_doesnt_override_error_json() {
        let app = Router::new().route(
            "/",
            get(|| async {
                let not_json_compatible = HashMap::from([(Vec::from([1, 2, 3]), 123)]);
                (StatusCode::IM_A_TEAPOT, Json(not_json_compatible))
            }),
        );

        let client = TestClient::new(app);

        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
