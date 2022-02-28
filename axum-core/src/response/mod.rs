//! Types and traits for generating responses.
//!
//! See [`axum::response`] for more details.
//!
//! [`axum::response`]: https://docs.rs/axum/latest/axum/response/index.html

use crate::{
    body::{self, BoxBody},
    BoxError,
};
use bytes::{buf::Chain, Buf, Bytes, BytesMut};
use http::{
    header::{self, HeaderMap, HeaderName, HeaderValue},
    StatusCode,
};
use http_body::{
    combinators::{MapData, MapErr},
    Empty, Full, SizeHint,
};
use std::{
    borrow::Cow,
    convert::{Infallible, TryInto},
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

mod generic_impls;

/// Type alias for [`http::Response`] whose body type defaults to [`BoxBody`], the most common body
/// type used with axum.
pub type Response<T = BoxBody> = http::Response<T>;

/// Trait for generating responses.
///
/// Types that implement `IntoResponse` can be returned from handlers.
pub trait IntoResponse {
    /// Create a response.
    fn into_response(self) -> Response;
}

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        *res.status_mut() = self;
        res
    }
}

impl IntoResponse for () {
    fn into_response(self) -> Response {
        Empty::new().into_response()
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
        self.map(body::boxed)
    }
}

impl IntoResponse for http::response::Parts {
    fn into_response(self) -> Response {
        Response::from_parts(self, body::boxed(Empty::new()))
    }
}

impl IntoResponse for Full<Bytes> {
    fn into_response(self) -> Response {
        Response::new(body::boxed(self))
    }
}

impl IntoResponse for Empty<Bytes> {
    fn into_response(self) -> Response {
        Response::new(body::boxed(self))
    }
}

impl<E> IntoResponse for http_body::combinators::BoxBody<Bytes, E>
where
    E: Into<BoxError> + 'static,
{
    fn into_response(self) -> Response {
        Response::new(body::boxed(self))
    }
}

impl<E> IntoResponse for http_body::combinators::UnsyncBoxBody<Bytes, E>
where
    E: Into<BoxError> + 'static,
{
    fn into_response(self) -> Response {
        Response::new(body::boxed(self))
    }
}

impl<B, F> IntoResponse for MapData<B, F>
where
    B: http_body::Body + Send + 'static,
    F: FnMut(B::Data) -> Bytes + Send + 'static,
    B::Error: Into<BoxError>,
{
    fn into_response(self) -> Response {
        Response::new(body::boxed(self))
    }
}

impl<B, F, E> IntoResponse for MapErr<B, F>
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    F: FnMut(B::Error) -> E + Send + 'static,
    E: Into<BoxError>,
{
    fn into_response(self) -> Response {
        Response::new(body::boxed(self))
    }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> Response {
        Cow::Borrowed(self).into_response()
    }
}

impl IntoResponse for String {
    fn into_response(self) -> Response {
        Cow::<'static, str>::Owned(self).into_response()
    }
}

impl IntoResponse for Cow<'static, str> {
    fn into_response(self) -> Response {
        let mut res = Full::from(self).into_response();
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
        );
        res
    }
}

impl IntoResponse for Bytes {
    fn into_response(self) -> Response {
        let mut res = Full::from(self).into_response();
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

impl IntoResponse for BytesMut {
    fn into_response(self) -> Response {
        self.freeze().into_response()
    }
}

impl<T, U> IntoResponse for Chain<T, U>
where
    T: Buf + Unpin + Send + 'static,
    U: Buf + Unpin + Send + 'static,
{
    fn into_response(self) -> Response {
        let (first, second) = self.into_inner();
        let mut res = Response::new(body::boxed(BytesChainBody {
            first: Some(first),
            second: Some(second),
        }));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

struct BytesChainBody<T, U> {
    first: Option<T>,
    second: Option<U>,
}

impl<T, U> http_body::Body for BytesChainBody<T, U>
where
    T: Buf + Unpin,
    U: Buf + Unpin,
{
    type Data = Bytes;
    type Error = Infallible;

    fn poll_data(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        if let Some(mut buf) = self.first.take() {
            let bytes = buf.copy_to_bytes(buf.remaining());
            return Poll::Ready(Some(Ok(bytes)));
        }

        if let Some(mut buf) = self.second.take() {
            let bytes = buf.copy_to_bytes(buf.remaining());
            return Poll::Ready(Some(Ok(bytes)));
        }

        Poll::Ready(None)
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }

    fn is_end_stream(&self) -> bool {
        self.first.is_none() && self.second.is_none()
    }

    fn size_hint(&self) -> SizeHint {
        match (self.first.as_ref(), self.second.as_ref()) {
            (Some(first), Some(second)) => {
                let total_size = first.remaining() + second.remaining();
                SizeHint::with_exact(total_size as u64)
            }
            (Some(buf), None) => SizeHint::with_exact(buf.remaining() as u64),
            (None, Some(buf)) => SizeHint::with_exact(buf.remaining() as u64),
            (None, None) => SizeHint::with_exact(0),
        }
    }
}

impl IntoResponse for &'static [u8] {
    fn into_response(self) -> Response {
        Cow::Borrowed(self).into_response()
    }
}

impl IntoResponse for Vec<u8> {
    fn into_response(self) -> Response {
        Cow::<'static, [u8]>::Owned(self).into_response()
    }
}

impl IntoResponse for Cow<'static, [u8]> {
    fn into_response(self) -> Response {
        let mut res = Full::from(self).into_response();
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

/// Trait for generating responses from individual parts.
///
/// # Implementing `IntoResponseParts`
///
/// You generally shouldn't have to implement `IntoResponseParts` manually, as axum
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
///     response::{IntoResponseParts, ResponseParts},
/// };
///
/// enum MyError {
///     SomethingWentWrong,
///     SomethingElseWentWrong,
/// }
///
/// impl IntoResponseParts for MyError {
///     fn into_response_parts(self, res: &mut ResponseParts) {
///         let body = match self {
///             MyError::SomethingWentWrong => {
///                 body::boxed(body::Full::from("something went wrong"))
///             },
///             MyError::SomethingElseWentWrong => {
///                 body::boxed(body::Full::from("something else went wrong"))
///             },
///         };
///
///         (StatusCode::INTERNAL_SERVER_ERROR, body).into_response_parts(res)
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
/// `IntoResponseParts` for it:
///
/// ```rust
/// use axum::{
///     body,
///     routing::get,
///     response::{IntoResponseParts, ResponseParts},
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
/// // Now we can implement `IntoResponseParts` directly for `MyBody`
/// impl IntoResponseParts for MyBody {
///     fn into_response_parts(self, res: &mut ResponseParts) {
///         res.set_body(self)
///     }
/// }
///
/// // `MyBody` can now be returned from handlers.
/// let app = Router::new().route("/", get(|| async { MyBody }));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub trait IntoResponseParts {
    /// Set parts of the response
    fn into_response_parts(self, res: &mut ResponseParts);
}

/// Parts of a response.
///
/// Used with [`IntoResponseParts`].
#[derive(Debug)]
pub struct ResponseParts {
    res: Result<Response, String>,
}

impl ResponseParts {
    /// Insert a header into the response.
    ///
    /// If the header already exists it will be overwritten.
    pub fn insert_header<K, V>(&mut self, key: K, value: V)
    where
        K: TryInto<HeaderName>,
        K::Error: fmt::Display,
        V: TryInto<HeaderValue>,
        V::Error: fmt::Display,
    {
        self.update_headers(key, value, |headers, key, value| {
            headers.insert(key, value);
        });
    }

    /// Append a header to the response.
    ///
    /// If the header already exists it will be appended to.
    pub fn append_header<K, V>(&mut self, key: K, value: V)
    where
        K: TryInto<HeaderName>,
        K::Error: fmt::Display,
        V: TryInto<HeaderValue>,
        V::Error: fmt::Display,
    {
        self.update_headers(key, value, |headers, key, value| {
            headers.append(key, value);
        });
    }

    fn update_headers<K, V, F>(&mut self, key: K, value: V, f: F)
    where
        K: TryInto<HeaderName>,
        K::Error: fmt::Display,
        V: TryInto<HeaderValue>,
        V::Error: fmt::Display,
        F: FnOnce(&mut HeaderMap, HeaderName, HeaderValue),
    {
        if let Ok(response) = &mut self.res {
            let key = match key.try_into() {
                Ok(key) => key,
                Err(err) => {
                    self.res = Err(err.to_string());
                    return;
                }
            };

            let value = match value.try_into() {
                Ok(value) => value,
                Err(err) => {
                    self.res = Err(err.to_string());
                    return;
                }
            };

            f(response.headers_mut(), key, value);
        }
    }

    /// Insert an extension into the response.
    ///
    /// If the extension already exists it will be overwritten.
    pub fn insert_extension<T>(&mut self, extension: T)
    where
        T: Send + Sync + 'static,
    {
        if let Ok(res) = &mut self.res {
            res.extensions_mut().insert(extension);
        }
    }
}

impl Extend<(Option<HeaderName>, HeaderValue)> for ResponseParts {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (Option<HeaderName>, HeaderValue)>,
    {
        if let Ok(res) = &mut self.res {
            res.headers_mut().extend(iter);
        }
    }
}

impl IntoResponseParts for HeaderMap {
    fn into_response_parts(self, res: &mut ResponseParts) {
        res.extend(self);
    }
}

impl<K, V, const N: usize> IntoResponseParts for [(K, V); N]
where
    K: TryInto<HeaderName>,
    K::Error: fmt::Display,
    V: TryInto<HeaderValue>,
    V::Error: fmt::Display,
{
    fn into_response_parts(self, res: &mut ResponseParts) {
        for (key, value) in self {
            res.insert_header(key, value);
        }
    }
}
