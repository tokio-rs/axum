#![doc = include_str!("../docs/response.md")]

use crate::{
    body::{boxed, BoxBody},
    BoxError,
};
use bytes::Bytes;
use http::{header, HeaderMap, HeaderValue, Response, StatusCode};
use http_body::{
    combinators::{MapData, MapErr},
    Empty, Full,
};
use std::{borrow::Cow, convert::Infallible};

mod headers;
mod redirect;

pub mod sse;

#[doc(no_inline)]
#[cfg(feature = "json")]
pub use crate::Json;

#[doc(inline)]
pub use self::{headers::Headers, redirect::Redirect, sse::Sse};

/// Trait for generating responses.
///
/// Types that implement `IntoResponse` can be returned from handlers.
///
/// # Implementing `IntoResponse`
///
/// You generally shouldn't have to implement `IntoResponse` manually, as axum
/// provides implementations for many common types.
///
/// However it might be necessary if you have a custom error type that you want
/// to return from handlers:
///
/// ```rust
/// use axum::{
///     Router,
///     body::{self, BoxBody, Bytes},
///     routing::get,
///     http::{Response, StatusCode},
///     response::IntoResponse,
/// };
///
/// enum MyError {
///     SomethingWentWrong,
///     SomethingElseWentWrong,
/// }
///
/// impl IntoResponse for MyError {
///     fn into_response(self) -> Response<BoxBody> {
///         let body = match self {
///             MyError::SomethingWentWrong => {
///                 body::boxed(body::Full::from("something went wrong"))
///             },
///             MyError::SomethingElseWentWrong => {
///                 body::boxed(body::Full::from("something else went wrong"))
///             },
///         };
///
///         Response::builder()
///             .status(StatusCode::INTERNAL_SERVER_ERROR)
///             .body(body)
///             .unwrap()
///     }
/// }
///
/// // `Result<impl IntoResponse, MyError>` can now be returned from handlers
/// let app = Router::new().route("/", get(handler));
///
/// async fn handler() -> Result<(), MyError> {
///     Err(MyError::SomethingWentWrong)
/// }
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Or if you have a custom body type you'll also need to implement
/// `IntoResponse` for it:
///
/// ```rust
/// use axum::{
///     body::{self, BoxBody},
///     routing::get,
///     response::IntoResponse,
///     Router,
/// };
/// use http_body::Body;
/// use http::{Response, HeaderMap};
/// use bytes::Bytes;
/// use std::{
///     convert::Infallible,
///     task::{Poll, Context},
///     pin::Pin,
/// };
///
/// struct MyBody;
///
/// // First implement `Body` for `MyBody`. This could for example use
/// // some custom streaming protocol.
/// impl Body for MyBody {
///     type Data = Bytes;
///     type Error = Infallible;
///
///     fn poll_data(
///         self: Pin<&mut Self>,
///         cx: &mut Context<'_>
///     ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
///         # unimplemented!()
///         // ...
///     }
///
///     fn poll_trailers(
///         self: Pin<&mut Self>,
///         cx: &mut Context<'_>
///     ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
///         # unimplemented!()
///         // ...
///     }
/// }
///
/// // Now we can implement `IntoResponse` directly for `MyBody`
/// impl IntoResponse for MyBody {
///     fn into_response(self) -> Response<BoxBody> {
///         Response::new(body::boxed(self))
///     }
/// }
///
/// // We don't need to implement `IntoResponse for Response<MyBody>` as that is
/// // covered by a blanket implementation in axum.
///
/// // `MyBody` can now be returned from handlers.
/// let app = Router::new().route("/", get(|| async { MyBody }));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub trait IntoResponse {
    /// Create a response.
    fn into_response(self) -> Response<BoxBody>;
}

impl IntoResponse for () {
    fn into_response(self) -> Response<BoxBody> {
        Response::new(boxed(Empty::new()))
    }
}

impl IntoResponse for Infallible {
    fn into_response(self) -> Response<BoxBody> {
        match self {}
    }
}

impl<T, E> IntoResponse for Result<T, E>
where
    T: IntoResponse,
    E: IntoResponse,
{
    fn into_response(self) -> Response<BoxBody> {
        match self {
            Ok(value) => value.into_response(),
            Err(err) => err.into_response(),
        }
    }
}

impl<B> IntoResponse for Response<B>
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    fn into_response(self) -> Response<BoxBody> {
        self.map(boxed)
    }
}

macro_rules! impl_into_response_for_body {
    ($body:ty) => {
        impl IntoResponse for $body {
            fn into_response(self) -> Response<BoxBody> {
                Response::new(boxed(self))
            }
        }
    };
}

impl_into_response_for_body!(hyper::Body);
impl_into_response_for_body!(Full<Bytes>);
impl_into_response_for_body!(Empty<Bytes>);

impl IntoResponse for http::response::Parts {
    fn into_response(self) -> Response<BoxBody> {
        Response::from_parts(self, boxed(Empty::new()))
    }
}

impl<E> IntoResponse for http_body::combinators::BoxBody<Bytes, E>
where
    E: Into<BoxError> + 'static,
{
    fn into_response(self) -> Response<BoxBody> {
        Response::new(boxed(self))
    }
}

impl<E> IntoResponse for http_body::combinators::UnsyncBoxBody<Bytes, E>
where
    E: Into<BoxError> + 'static,
{
    fn into_response(self) -> Response<BoxBody> {
        Response::new(boxed(self))
    }
}

impl<B, F> IntoResponse for MapData<B, F>
where
    B: http_body::Body + Send + 'static,
    F: FnMut(B::Data) -> Bytes + Send + 'static,
    B::Error: Into<BoxError>,
{
    fn into_response(self) -> Response<BoxBody> {
        Response::new(boxed(self))
    }
}

impl<B, F, E> IntoResponse for MapErr<B, F>
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    F: FnMut(B::Error) -> E + Send + 'static,
    E: Into<BoxError>,
{
    fn into_response(self) -> Response<BoxBody> {
        Response::new(boxed(self))
    }
}

impl IntoResponse for &'static str {
    #[inline]
    fn into_response(self) -> Response<BoxBody> {
        Cow::Borrowed(self).into_response()
    }
}

impl IntoResponse for String {
    #[inline]
    fn into_response(self) -> Response<BoxBody> {
        Cow::<'static, str>::Owned(self).into_response()
    }
}

impl IntoResponse for Cow<'static, str> {
    fn into_response(self) -> Response<BoxBody> {
        let mut res = Response::new(boxed(Full::from(self)));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
        );
        res
    }
}

impl IntoResponse for Bytes {
    fn into_response(self) -> Response<BoxBody> {
        let mut res = Response::new(boxed(Full::from(self)));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

impl IntoResponse for &'static [u8] {
    fn into_response(self) -> Response<BoxBody> {
        let mut res = Response::new(boxed(Full::from(self)));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

impl IntoResponse for Vec<u8> {
    fn into_response(self) -> Response<BoxBody> {
        let mut res = Response::new(boxed(Full::from(self)));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

impl IntoResponse for Cow<'static, [u8]> {
    fn into_response(self) -> Response<BoxBody> {
        let mut res = Response::new(boxed(Full::from(self)));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response<BoxBody> {
        Response::builder()
            .status(self)
            .body(boxed(Empty::new()))
            .unwrap()
    }
}

impl<T> IntoResponse for (StatusCode, T)
where
    T: IntoResponse,
{
    fn into_response(self) -> Response<BoxBody> {
        let mut res = self.1.into_response();
        *res.status_mut() = self.0;
        res
    }
}

impl<T> IntoResponse for (HeaderMap, T)
where
    T: IntoResponse,
{
    fn into_response(self) -> Response<BoxBody> {
        let mut res = self.1.into_response();
        res.headers_mut().extend(self.0);
        res
    }
}

impl<T> IntoResponse for (StatusCode, HeaderMap, T)
where
    T: IntoResponse,
{
    fn into_response(self) -> Response<BoxBody> {
        let mut res = self.2.into_response();
        *res.status_mut() = self.0;
        res.headers_mut().extend(self.1);
        res
    }
}

impl IntoResponse for HeaderMap {
    fn into_response(self) -> Response<BoxBody> {
        let mut res = Response::new(boxed(Empty::new()));
        *res.headers_mut() = self;
        res
    }
}

/// An HTML response.
///
/// Will automatically get `Content-Type: text/html`.
#[derive(Clone, Copy, Debug)]
pub struct Html<T>(pub T);

impl<T> IntoResponse for Html<T>
where
    T: Into<Full<Bytes>>,
{
    fn into_response(self) -> Response<BoxBody> {
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
    use http::header::{HeaderMap, HeaderName};

    #[test]
    fn test_merge_headers() {
        struct MyResponse;

        impl IntoResponse for MyResponse {
            fn into_response(self) -> Response<BoxBody> {
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
