//! Types and traits for generating responses.
//!
//! See [`axum::response`] for more details.
//!
//! [`axum::response`]: https://docs.rs/axum/latest/axum/response/index.html

use crate::{
    body::{boxed, BoxBody},
    BoxError, Error,
};
use bytes::Bytes;
use http::{
    header::{self, HeaderMap, HeaderValue},
    Response, StatusCode,
};
use http_body::{
    combinators::{MapData, MapErr},
    Empty, Full,
};
use std::{borrow::Cow, convert::Infallible};

mod headers;

#[doc(inline)]
pub use self::headers::Headers;

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
///     body::Body,
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
///     type Body = Body;
///     type BodyError = <Self::Body as axum::body::HttpBody>::Error;
///
///     fn into_response(self) -> Response<Self::Body> {
///         let body = match self {
///             MyError::SomethingWentWrong => {
///                 Body::from("something went wrong")
///             },
///             MyError::SomethingElseWentWrong => {
///                 Body::from("something else went wrong")
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
/// let app = Router::new().route("/", get(|| async { MyBody }));
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
    /// [`boxed`].
    ///
    /// [`axum::body::Body`]: https://docs.rs/axum/latest/axum/body/index.html
    /// [`axum::body::Empty<Bytes>`]: https://docs.rs/axum/latest/axum/body/index.html
    /// [`axum::body::Full<Bytes>`]: https://docs.rs/axum/latest/axum/body/index.html
    /// [`axum::body::BoxBody`]: https://docs.rs/axum/latest/axum/body/index.html
    type Body: http_body::Body<Data = Bytes, Error = Self::BodyError> + Send + 'static;

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

impl<T, E> IntoResponse for Result<T, E>
where
    T: IntoResponse,
    E: IntoResponse,
{
    type Body = BoxBody;
    type BodyError = Error;

    fn into_response(self) -> Response<Self::Body> {
        match self {
            Ok(value) => value.into_response().map(boxed),
            Err(err) => err.into_response().map(boxed),
        }
    }
}

impl<B> IntoResponse for Response<B>
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
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

impl IntoResponse for http::response::Parts {
    type Body = Empty<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        Response::from_parts(self, Empty::new())
    }
}

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

impl<E> IntoResponse for http_body::combinators::UnsyncBoxBody<Bytes, E>
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
    B: http_body::Body + Send + 'static,
    F: FnMut(B::Data) -> Bytes + Send + 'static,
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
    B: http_body::Body<Data = Bytes> + Send + 'static,
    F: FnMut(B::Error) -> E + Send + 'static,
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
        #[allow(clippy::declare_interior_mutable_const)]
        const TEXT_PLAIN: HeaderValue = HeaderValue::from_static("text/plain");

        let mut res = Response::new(Full::from(self));
        res.headers_mut().insert(header::CONTENT_TYPE, TEXT_PLAIN);
        res
    }
}

#[allow(clippy::declare_interior_mutable_const)]
const APPLICATION_OCTET_STREAM: HeaderValue = HeaderValue::from_static("application/octet-stream");

impl IntoResponse for Bytes {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Full::from(self));
        res.headers_mut()
            .insert(header::CONTENT_TYPE, APPLICATION_OCTET_STREAM);
        res
    }
}

impl IntoResponse for &'static [u8] {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Full::from(self));
        res.headers_mut()
            .insert(header::CONTENT_TYPE, APPLICATION_OCTET_STREAM);
        res
    }
}

impl IntoResponse for Vec<u8> {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Full::from(self));
        res.headers_mut()
            .insert(header::CONTENT_TYPE, APPLICATION_OCTET_STREAM);
        res
    }
}

impl IntoResponse for std::borrow::Cow<'static, [u8]> {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let mut res = Response::new(Full::from(self));
        res.headers_mut()
            .insert(header::CONTENT_TYPE, APPLICATION_OCTET_STREAM);
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
