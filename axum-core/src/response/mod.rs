//! Types and traits for generating responses.
//!
//! See [`axum::response`] for more details.
//!
//! [`axum::response`]: https://docs.rs/axum/latest/axum/response/index.html

use crate::{
    body::{boxed, BoxBody},
    BoxError,
};
use bytes::Bytes;
use http::{
    header::{self, HeaderMap, HeaderName, HeaderValue},
    StatusCode,
};
use http_body::{
    combinators::{MapData, MapErr},
    Empty, Full,
};
use std::{borrow::Cow, convert::Infallible, iter};

mod headers;

#[doc(inline)]
pub use self::headers::Headers;

/// Type alias for [`http::Response`] whose body type defaults to [`BoxBody`], the most common body
/// type used with axum.
pub type Response<T = BoxBody> = http::Response<T>;

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
///     body::{self, Bytes},
///     routing::get,
///     http::StatusCode,
///     response::{IntoResponse, Response},
/// };
///
/// enum MyError {
///     SomethingWentWrong,
///     SomethingElseWentWrong,
/// }
///
/// impl IntoResponse for MyError {
///     fn into_response(self) -> Response {
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
///     body,
///     routing::get,
///     response::{IntoResponse, Response},
///     Router,
/// };
/// use http_body::Body;
/// use http::HeaderMap;
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
///     fn into_response(self) -> Response {
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
    fn into_response(self) -> Response;
}

/// Trait for generating response headers.
///
/// Any type that implements this trait automatically implements `IntoResponse` as well, but can
/// also be used in a tuple like `(StatusCode, Self)`, `(Self, impl IntoResponseHeaders)`,
/// `(StatusCode, Self, impl IntoResponseHeaders, impl IntoResponse)` and so on.
///
/// This trait can't currently be implemented outside of axum.
pub trait IntoResponseHeaders {
    /// The return type of [`.into_headers()`].
    ///
    /// The iterator item is a `Result` to allow the implementation to return a server error
    /// instead.
    ///
    /// The header name is optional because `HeaderMap`s iterator doesn't yield it multiple times
    /// for headers that have multiple values, to avoid unnecessary copies.
    #[doc(hidden)]
    type IntoIter: IntoIterator<Item = Result<(Option<HeaderName>, HeaderValue), Response>>;

    /// Attempt to turn `self` into a list of headers.
    ///
    /// In practice, only the implementation for `axum::response::Headers` ever returns `Err(_)`.
    #[doc(hidden)]
    fn into_headers(self) -> Self::IntoIter;
}

impl IntoResponse for () {
    fn into_response(self) -> Response {
        Response::new(boxed(Empty::new()))
    }
}

impl IntoResponse for Infallible {
    fn into_response(self) -> Response {
        match self {}
    }
}

impl<T, E> IntoResponse for Result<T, E>
where
    T: IntoResponse,
    E: IntoResponse,
{
    fn into_response(self) -> Response {
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
    fn into_response(self) -> Response {
        self.map(boxed)
    }
}

macro_rules! impl_into_response_for_body {
    ($body:ty) => {
        impl IntoResponse for $body {
            fn into_response(self) -> Response {
                Response::new(boxed(self))
            }
        }
    };
}

impl_into_response_for_body!(Full<Bytes>);
impl_into_response_for_body!(Empty<Bytes>);

impl IntoResponse for http::response::Parts {
    fn into_response(self) -> Response {
        Response::from_parts(self, boxed(Empty::new()))
    }
}

impl<E> IntoResponse for http_body::combinators::BoxBody<Bytes, E>
where
    E: Into<BoxError> + 'static,
{
    fn into_response(self) -> Response {
        Response::new(boxed(self))
    }
}

impl<E> IntoResponse for http_body::combinators::UnsyncBoxBody<Bytes, E>
where
    E: Into<BoxError> + 'static,
{
    fn into_response(self) -> Response {
        Response::new(boxed(self))
    }
}

impl<B, F> IntoResponse for MapData<B, F>
where
    B: http_body::Body + Send + 'static,
    F: FnMut(B::Data) -> Bytes + Send + 'static,
    B::Error: Into<BoxError>,
{
    fn into_response(self) -> Response {
        Response::new(boxed(self))
    }
}

impl<B, F, E> IntoResponse for MapErr<B, F>
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    F: FnMut(B::Error) -> E + Send + 'static,
    E: Into<BoxError>,
{
    fn into_response(self) -> Response {
        Response::new(boxed(self))
    }
}

impl IntoResponse for &'static str {
    #[inline]
    fn into_response(self) -> Response {
        Cow::Borrowed(self).into_response()
    }
}

impl IntoResponse for String {
    #[inline]
    fn into_response(self) -> Response {
        Cow::<'static, str>::Owned(self).into_response()
    }
}

impl IntoResponse for Cow<'static, str> {
    fn into_response(self) -> Response {
        let mut res = Response::new(boxed(Full::from(self)));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
        );
        res
    }
}

impl IntoResponse for Bytes {
    fn into_response(self) -> Response {
        let mut res = Response::new(boxed(Full::from(self)));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

impl IntoResponse for &'static [u8] {
    fn into_response(self) -> Response {
        let mut res = Response::new(boxed(Full::from(self)));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

impl IntoResponse for Vec<u8> {
    fn into_response(self) -> Response {
        let mut res = Response::new(boxed(Full::from(self)));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

impl IntoResponse for Cow<'static, [u8]> {
    fn into_response(self) -> Response {
        let mut res = Response::new(boxed(Full::from(self)));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        Response::builder()
            .status(self)
            .body(boxed(Empty::new()))
            .unwrap()
    }
}

impl<H> IntoResponse for H
where
    H: IntoResponseHeaders,
{
    fn into_response(self) -> Response {
        let mut res = Response::new(boxed(Empty::new()));

        if let Err(e) = try_extend_headers(res.headers_mut(), self.into_headers()) {
            return e;
        }

        res
    }
}

impl<T> IntoResponse for (StatusCode, T)
where
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut res = self.1.into_response();
        *res.status_mut() = self.0;
        res
    }
}

impl<H, T> IntoResponse for (H, T)
where
    H: IntoResponseHeaders,
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut res = self.1.into_response();

        if let Err(e) = try_extend_headers(res.headers_mut(), self.0.into_headers()) {
            return e;
        }

        res
    }
}

impl<H, T> IntoResponse for (StatusCode, H, T)
where
    H: IntoResponseHeaders,
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut res = self.2.into_response();
        *res.status_mut() = self.0;

        if let Err(e) = try_extend_headers(res.headers_mut(), self.1.into_headers()) {
            return e;
        }

        res
    }
}

impl IntoResponseHeaders for HeaderMap {
    // FIXME: Use type_alias_impl_trait when available
    type IntoIter = iter::Map<
        http::header::IntoIter<HeaderValue>,
        fn(
            (Option<HeaderName>, HeaderValue),
        ) -> Result<(Option<HeaderName>, HeaderValue), Response>,
    >;

    fn into_headers(self) -> Self::IntoIter {
        self.into_iter().map(Ok)
    }
}

// Slightly adjusted version of `impl<T> Extend<(Option<HeaderName>, T)> for HeaderMap<T>`.
// Accepts an iterator that returns Results and short-circuits on an `Err`.
fn try_extend_headers(
    headers: &mut HeaderMap,
    iter: impl IntoIterator<Item = Result<(Option<HeaderName>, HeaderValue), Response>>,
) -> Result<(), Response> {
    use http::header::Entry;

    let mut iter = iter.into_iter();

    // The structure of this is a bit weird, but it is mostly to make the
    // borrow checker happy.
    let (mut key, mut val) = match iter.next().transpose()? {
        Some((Some(key), val)) => (key, val),
        Some((None, _)) => panic!("expected a header name, but got None"),
        None => return Ok(()),
    };

    'outer: loop {
        let mut entry = match headers.entry(key) {
            Entry::Occupied(mut e) => {
                // Replace all previous values while maintaining a handle to
                // the entry.
                e.insert(val);
                e
            }
            Entry::Vacant(e) => e.insert_entry(val),
        };

        // As long as `HeaderName` is none, keep inserting the value into
        // the current entry
        loop {
            match iter.next().transpose()? {
                Some((Some(k), v)) => {
                    key = k;
                    val = v;
                    continue 'outer;
                }
                Some((None, v)) => {
                    entry.append(v);
                }
                None => {
                    return Ok(());
                }
            }
        }
    }
}
