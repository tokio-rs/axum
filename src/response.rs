//! Types and traits for generating responses.

use crate::{
    body::{box_body, BoxBody},
    Error,
};
use bytes::Bytes;
use http::{header, HeaderMap, HeaderValue, Response, StatusCode};
use http_body::{
    combinators::{MapData, MapErr},
    Empty, Full,
};
use std::{borrow::Cow, convert::Infallible};
use tower::{util::Either, BoxError};

#[doc(no_inline)]
pub use crate::Json;

/// Trait for generating responses.
///
/// Types that implement `IntoResponse` can be returned from handlers.
///
/// # Implementing `IntoResponse`
///
/// You generally shouldn't have to implement `IntoResponse` manually, as axum
/// provides implementations for many common types.
///
/// A manual implementation should only be necessary if you have a custom
/// response body type:
///
/// ```rust
/// use axum::{prelude::*, response::IntoResponse};
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
///     type Body = Self;
///     type BodyError = <Self as Body>::Error;
///
///     fn into_response(self) -> Response<Self::Body> {
///         Response::new(self)
///     }
/// }
///
/// // We don't need to implement `IntoResponse for Response<MyBody>` as that is
/// // covered by a blanket implementation in axum.
///
/// // `MyBody` can now be returned from handlers.
/// let app = route("/", get(|| async { MyBody }));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub trait IntoResponse {
    /// The body type of the response.
    ///
    /// Unless you're implementing this trait for a custom body type, these are
    /// some common types you can use:
    ///
    /// - [`axum::body::Body`]: A good default that supports most use cases.
    /// - [`axum::body::Empty<Bytes>`]: When you know your response is always
    /// empty.
    /// - [`axum::body::Full<Bytes>`]: When you know your response always
    /// contains exactly one chunk.
    /// - [`axum::body::BoxBody`]: If you need to unify multiple body types into
    /// one, or return a body type that cannot be named. Can be created with
    /// [`box_body`].
    ///
    /// [`axum::body::Body`]: crate::body::Body
    /// [`axum::body::Empty<Bytes>`]: crate::body::Empty
    /// [`axum::body::Full<Bytes>`]: crate::body::Full
    /// [`axum::body::BoxBody`]: crate::body::BoxBody
    type Body: http_body::Body<Data = Bytes, Error = Self::BodyError> + Send + Sync + 'static;

    /// The error type `Self::Body` might generate.
    ///
    /// Generally it should be possible to set this to:
    ///
    /// ```rust,ignore
    /// type BodyError = <Self::Body as axum::body::HttpBody>::Error;
    /// ```
    ///
    /// This associated type exists mainly to make returning `impl IntoResponse`
    /// possible and to simplify trait bounds internally in axum.
    type BodyError: Into<BoxError>;

    /// Create a response.
    fn into_response(self) -> Response<Self::Body>;
}

impl IntoResponse for () {
    type Body = Empty<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        Response::new(Empty::new())
    }
}

impl IntoResponse for Infallible {
    type Body = Empty<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        match self {}
    }
}

impl<T, K> IntoResponse for Either<T, K>
where
    T: IntoResponse,
    K: IntoResponse,
{
    type Body = BoxBody;
    type BodyError = Error;

    fn into_response(self) -> Response<Self::Body> {
        match self {
            Either::A(inner) => inner.into_response().map(box_body),
            Either::B(inner) => inner.into_response().map(box_body),
        }
    }
}

impl<T, E> IntoResponse for Result<T, E>
where
    T: IntoResponse,
    E: IntoResponse,
{
    type Body = BoxBody;
    type BodyError = Error;

    fn into_response(self) -> Response<Self::Body> {
        match self {
            Ok(value) => value.into_response().map(box_body),
            Err(err) => err.into_response().map(box_body),
        }
    }
}

impl<B> IntoResponse for Response<B>
where
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError>,
{
    type Body = B;
    type BodyError = <B as http_body::Body>::Error;

    fn into_response(self) -> Self {
        self
    }
}

macro_rules! impl_into_response_for_body {
    ($body:ty) => {
        impl IntoResponse for $body {
            type Body = $body;
            type BodyError = <$body as http_body::Body>::Error;

            fn into_response(self) -> Response<Self> {
                Response::new(self)
            }
        }
    };
}

impl_into_response_for_body!(hyper::Body);
impl_into_response_for_body!(Full<Bytes>);
impl_into_response_for_body!(Empty<Bytes>);

impl<E> IntoResponse for http_body::combinators::BoxBody<Bytes, E>
where
    E: Into<BoxError> + 'static,
{
    type Body = Self;
    type BodyError = E;

    fn into_response(self) -> Response<Self> {
        Response::new(self)
    }
}

impl<B, F> IntoResponse for MapData<B, F>
where
    B: http_body::Body + Send + Sync + 'static,
    F: FnMut(B::Data) -> Bytes + Send + Sync + 'static,
    B::Error: Into<BoxError>,
{
    type Body = Self;
    type BodyError = <B as http_body::Body>::Error;

    fn into_response(self) -> Response<Self::Body> {
        Response::new(self)
    }
}

impl<B, F, E> IntoResponse for MapErr<B, F>
where
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    F: FnMut(B::Error) -> E + Send + Sync + 'static,
    E: Into<BoxError>,
{
    type Body = Self;
    type BodyError = E;

    fn into_response(self) -> Response<Self::Body> {
        Response::new(self)
    }
}

impl IntoResponse for &'static str {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    #[inline]
    fn into_response(self) -> Response<Self::Body> {
        Cow::Borrowed(self).into_response()
    }
}

impl IntoResponse for String {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    #[inline]
    fn into_response(self) -> Response<Self::Body> {
        Cow::<'static, str>::Owned(self).into_response()
    }
}

impl IntoResponse for std::borrow::Cow<'static, str> {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Full::from(self));
        res.headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
        res
    }
}

impl IntoResponse for Bytes {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Full::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse for &'static [u8] {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Full::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse for Vec<u8> {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Full::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse for std::borrow::Cow<'static, [u8]> {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Full::from(self));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse for StatusCode {
    type Body = Empty<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        Response::builder().status(self).body(Empty::new()).unwrap()
    }
}

impl<T> IntoResponse for (StatusCode, T)
where
    T: IntoResponse,
{
    type Body = T::Body;
    type BodyError = T::BodyError;

    fn into_response(self) -> Response<T::Body> {
        let mut res = self.1.into_response();
        *res.status_mut() = self.0;
        res
    }
}

impl<T> IntoResponse for (HeaderMap, T)
where
    T: IntoResponse,
{
    type Body = T::Body;
    type BodyError = T::BodyError;

    fn into_response(self) -> Response<T::Body> {
        let mut res = self.1.into_response();
        res.headers_mut().extend(self.0);
        res
    }
}

impl<T> IntoResponse for (StatusCode, HeaderMap, T)
where
    T: IntoResponse,
{
    type Body = T::Body;
    type BodyError = T::BodyError;

    fn into_response(self) -> Response<T::Body> {
        let mut res = self.2.into_response();
        *res.status_mut() = self.0;
        res.headers_mut().extend(self.1);
        res
    }
}

impl IntoResponse for HeaderMap {
    type Body = Empty<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Empty::new());
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
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(self.0.into());
        res.headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html"));
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
    use http::header::{HeaderMap, HeaderName};

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
