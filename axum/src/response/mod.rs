//! Types and traits for generating responses.
//!
//! # Building responses
//!
//! Anything that implements [`IntoResponse`] can be returned from a handler. axum
//! provides implementations for common types:
//!
//! ```rust,no_run
//! use axum::{
//!     Json,
//!     response::{Html, IntoResponse},
//!     http::{StatusCode, Uri, header::{self, HeaderMap, HeaderName}},
//! };
//!
//! // `()` gives an empty response
//! async fn empty() {}
//!
//! // String will get a `text/plain; charset=utf-8` content-type
//! async fn plain_text(uri: Uri) -> String {
//!     format!("Hi from {}", uri.path())
//! }
//!
//! // Bytes will get a `application/octet-stream` content-type
//! async fn bytes() -> Vec<u8> {
//!     vec![1, 2, 3, 4]
//! }
//!
//! // `Json` will get a `application/json` content-type and work with anything that
//! // implements `serde::Serialize`
//! async fn json() -> Json<Vec<String>> {
//!     Json(vec!["foo".to_owned(), "bar".to_owned()])
//! }
//!
//! // `Html` will get a `text/html` content-type
//! async fn html() -> Html<&'static str> {
//!     Html("<p>Hello, World!</p>")
//! }
//!
//! // `StatusCode` gives an empty response with that status code
//! async fn status() -> StatusCode {
//!     StatusCode::NOT_FOUND
//! }
//!
//! // `HeaderMap` gives an empty response with some headers
//! async fn headers() -> HeaderMap {
//!     let mut headers = HeaderMap::new();
//!     headers.insert(header::SERVER, "axum".parse().unwrap());
//!     headers
//! }
//!
//! // An array of tuples also gives headers
//! async fn array_headers() -> [(HeaderName, &'static str); 2] {
//!     [
//!         (header::SERVER, "axum"),
//!         (header::CONTENT_TYPE, "text/plain")
//!     ]
//! }
//!
//! // Use `impl IntoResponse` to avoid writing the whole type
//! async fn impl_trait() -> impl IntoResponse {
//!     [
//!         (header::SERVER, "axum"),
//!         (header::CONTENT_TYPE, "text/plain")
//!     ]
//! }
//! ```
//!
//! Additionally you can return tuples to build more complex responses from
//! individual parts.
//!
//! ```rust,no_run
//! use axum::{
//!     Json,
//!     response::IntoResponse,
//!     http::{StatusCode, HeaderMap, Uri, header},
//!     extract::Extension,
//! };
//!
//! // `(StatusCode, impl IntoResponse)` will override the status code of the response
//! async fn with_status(uri: Uri) -> (StatusCode, String) {
//!     (StatusCode::NOT_FOUND, format!("Not Found: {}", uri.path()))
//! }
//!
//! // Use `impl IntoResponse` to avoid having to type the whole type
//! async fn impl_trait(uri: Uri) -> impl IntoResponse {
//!     (StatusCode::NOT_FOUND, format!("Not Found: {}", uri.path()))
//! }
//!
//! // `(HeaderMap, impl IntoResponse)` to add additional headers
//! async fn with_headers() -> impl IntoResponse {
//!     let mut headers = HeaderMap::new();
//!     headers.insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());
//!     (headers, "foo")
//! }
//!
//! // Or an array of tuples to more easily build the headers
//! async fn with_array_headers() -> impl IntoResponse {
//!     ([(header::CONTENT_TYPE, "text/plain")], "foo")
//! }
//!
//! // Use string keys for custom headers
//! async fn with_array_headers_custom() -> impl IntoResponse {
//!     ([("x-custom", "custom")], "foo")
//! }
//!
//! // `(StatusCode, headers, impl IntoResponse)` to set status and add headers
//! // `headers` can be either a `HeaderMap` or an array of tuples
//! async fn with_status_and_array_headers() -> impl IntoResponse {
//!     (
//!         StatusCode::NOT_FOUND,
//!         [(header::CONTENT_TYPE, "text/plain")],
//!         "foo",
//!     )
//! }
//!
//! // `(Extension<_>, impl IntoResponse)` to set response extensions
//! async fn with_status_extensions() -> impl IntoResponse {
//!     (
//!         Extension(Foo("foo")),
//!         "foo",
//!     )
//! }
//!
//! #[derive(Clone)]
//! struct Foo(&'static str);
//!
//! // Or mix and match all the things
//! async fn all_the_things(uri: Uri) -> impl IntoResponse {
//!     let mut header_map = HeaderMap::new();
//!     if uri.path() == "/" {
//!         header_map.insert(header::SERVER, "axum".parse().unwrap());
//!     }
//!
//!     (
//!         // set status code
//!         StatusCode::NOT_FOUND,
//!         // headers with an array
//!         [("x-custom", "custom")],
//!         // some extensions
//!         Extension(Foo("foo")),
//!         Extension(Foo("bar")),
//!         // more headers, built dynamically
//!         header_map,
//!         // and finally the body
//!         "foo",
//!     )
//! }
//! ```
//!
//! In general you can return tuples like:
//!
//! - `(StatusCode, impl IntoResponse)`
//! - `(Parts, impl IntoResponse)`
//! - `(Response<()>, impl IntoResponse)`
//! - `(T1, .., Tn, impl IntoResponse)` where `T1` to `Tn` all implement [`IntoResponseParts`].
//! - `(StatusCode, T1, .., Tn, impl IntoResponse)` where `T1` to `Tn` all implement [`IntoResponseParts`].
//! - `(Parts, T1, .., Tn, impl IntoResponse)` where `T1` to `Tn` all implement [`IntoResponseParts`].
//! - `(Response<()>, T1, .., Tn, impl IntoResponse)` where `T1` to `Tn` all implement [`IntoResponseParts`].
//!
//! This means you cannot accidentally override the status or body as [`IntoResponseParts`] only allows
//! setting headers and extensions.
//!
//! Use [`Response`] for more low level control:
//!
//! ```rust,no_run
//! use axum::{
//!     Json,
//!     response::{IntoResponse, Response},
//!     body::Body,
//!     http::StatusCode,
//! };
//!
//! async fn response() -> Response {
//!     Response::builder()
//!         .status(StatusCode::NOT_FOUND)
//!         .header("x-foo", "custom header")
//!         .body(Body::from("not found"))
//!         .unwrap()
//! }
//! ```
//!
//! # Returning different response types
//!
//! If you need to return multiple response types, and `Result<T, E>` isn't appropriate, you can call
//! `.into_response()` to turn things into `axum::response::Response`:
//!
//! ```rust
//! use axum::{
//!     response::{IntoResponse, Redirect, Response},
//!     http::StatusCode,
//! };
//!
//! async fn handle() -> Response {
//!     if something() {
//!         "All good!".into_response()
//!     } else if something_else() {
//!         (
//!             StatusCode::INTERNAL_SERVER_ERROR,
//!             "Something went wrong...",
//!         ).into_response()
//!     } else {
//!         Redirect::to("/").into_response()
//!     }
//! }
//!
//! fn something() -> bool {
//!     // ...
//!     # true
//! }
//!
//! fn something_else() -> bool {
//!     // ...
//!     # true
//! }
//! ```
//!
//! # Regarding `impl IntoResponse`
//!
//! You can use `impl IntoResponse` as the return type from handlers to avoid
//! typing large types. For example
//!
//! ```rust
//! use axum::http::StatusCode;
//!
//! async fn handler() -> (StatusCode, [(&'static str, &'static str); 1], &'static str) {
//!     (StatusCode::OK, [("x-foo", "bar")], "Hello, World!")
//! }
//! ```
//!
//! Becomes easier using `impl IntoResponse`:
//!
//! ```rust
//! use axum::{http::StatusCode, response::IntoResponse};
//!
//! async fn impl_into_response() -> impl IntoResponse {
//!     (StatusCode::OK, [("x-foo", "bar")], "Hello, World!")
//! }
//! ```
//!
//! However `impl IntoResponse` has a few limitations. Firstly it can only be used
//! to return a single type:
//!
//! ```rust,compile_fail
//! use axum::{http::StatusCode, response::IntoResponse};
//!
//! async fn handler() -> impl IntoResponse {
//!     if check_something() {
//!         StatusCode::NOT_FOUND
//!     } else {
//!         "Hello, World!"
//!     }
//! }
//!
//! fn check_something() -> bool {
//!     # false
//!     // ...
//! }
//! ```
//!
//! This function returns either a `StatusCode` or a `&'static str` which `impl
//! Trait` doesn't allow.
//!
//! Secondly `impl IntoResponse` can lead to type inference issues when used with
//! `Result` and `?`:
//!
//! ```rust,compile_fail
//! use axum::{http::StatusCode, response::IntoResponse};
//!
//! async fn handler() -> impl IntoResponse {
//!     create_thing()?;
//!     Ok(StatusCode::CREATED)
//! }
//!
//! fn create_thing() -> Result<(), StatusCode> {
//!     # Ok(())
//!     // ...
//! }
//! ```
//!
//! This is because `?` supports using the [`From`] trait to convert to a different
//! error type but it doesn't know which type to convert to, because we only
//! specified `impl IntoResponse` as the return type.
//!
//! `Result<impl IntoResponse, impl IntoResponse>` doesn't always work either:
//!
//! ```rust,compile_fail
//! use axum::{http::StatusCode, response::IntoResponse};
//!
//! async fn handler() -> Result<impl IntoResponse, impl IntoResponse> {
//!     create_thing()?;
//!     Ok(StatusCode::CREATED)
//! }
//!
//! fn create_thing() -> Result<(), StatusCode> {
//!     # Ok(())
//!     // ...
//! }
//! ```
//!
//! The solution is to use a concrete error type, such as `Result<impl IntoResponse, StatusCode>`:
//!
//! ```rust
//! use axum::{http::StatusCode, response::IntoResponse};
//!
//! async fn handler() -> Result<impl IntoResponse, StatusCode> {
//!     create_thing()?;
//!     Ok(StatusCode::CREATED)
//! }
//!
//! fn create_thing() -> Result<(), StatusCode> {
//!     # Ok(())
//!     // ...
//! }
//! ```
//!
//! Because of this it is generally not recommended to use `impl IntoResponse`
//! unless you're familiar with the details of how `impl Trait` works.
//!
//! [`IntoResponse`]: crate::response::IntoResponse
//! [`IntoResponseParts`]: crate::response::IntoResponseParts
//! [`StatusCode`]: http::StatusCode

use http::{header, HeaderValue, StatusCode};

mod redirect;

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
pub use sse::Sse;

/// An HTML response.
///
/// Will automatically get `Content-Type: text/html`.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct Html<T>(pub T);

impl<T> IntoResponse for Html<T>
where
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static(mime::TEXT_HTML_UTF_8.as_ref()),
            )],
            self.0,
        )
            .into_response()
    }
}

impl<T> From<T> for Html<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

/// An empty response with 204 No Content status.
///
/// Due to historical and implementation reasons, the `IntoResponse` implementation of `()`
/// (unit type) returns an empty response with 200 [`StatusCode::OK`] status.
/// If you specifically want a 204 [`StatusCode::NO_CONTENT`] status, you can use either `StatusCode` type
/// directly, or this shortcut struct for self-documentation.
///
/// ```
/// use axum::{extract::Path, response::NoContent};
///
/// async fn delete_user(Path(user): Path<String>) -> Result<NoContent, String> {
///     // ...access database...
/// # drop(user);
///     Ok(NoContent)
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct NoContent;

impl IntoResponse for NoContent {
    fn into_response(self) -> Response {
        StatusCode::NO_CONTENT.into_response()
    }
}

#[cfg(test)]
mod tests {
    use crate::extract::Extension;
    use crate::{routing::get, Router};
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
    fn no_content() {
        assert_eq!(
            super::NoContent.into_response().status(),
            StatusCode::NO_CONTENT,
        )
    }
}
