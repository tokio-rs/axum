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
    Extensions, StatusCode, Version,
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

/// Type alias for [`http::Response`] whose body type defaults to [`BoxBody`], the most common body
/// type used with axum.
pub type Response<T = BoxBody> = http::Response<T>;

/// Trait for generating responses.
///
/// Types that implement `IntoResponse` can be returned from handlers.
pub trait IntoResponse {
    /// Create a response.
    fn into_response(self) -> Response;

    /// This method seals `IntoResponse` as it contains a type that cannot be named. This is
    /// intentional.
    ///
    /// Instead you must implement [`IntoResponseParts`] instead and rely on the blanket impl
    /// `impl<T: IntoResponseParts> IntoResponse for T`. This means that any type that implements
    /// [`IntoResponseParts`] also automatically implements `IntoResponse`.
    // This method contains a type that cannot be named outside axum-core, thus sealing the trait.
    // We cannot use a sealed super trait since we a blanket impl.
    //
    // The method is intentionally public so users will see it when they open the docs.
    fn sealed(_: sealed::DontImplementThisTrait);
}

mod sealed {
    #![allow(unreachable_pub, missing_debug_implementations)]

    pub struct DontImplementThisTrait;
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

impl<T> IntoResponse for T
where
    T: IntoResponseParts,
{
    fn into_response(self) -> Response {
        let mut parts = ResponseParts {
            version: None,
            status: StatusCode::OK,
            headers: Ok(HeaderMap::new()),
            extensions: Extensions::new(),
            body: None,
        };

        self.into_response_parts(&mut parts);

        let ResponseParts {
            version,
            status,
            headers,
            extensions,
            body,
        } = parts;

        let headers = match headers {
            Ok(headers) => headers,
            Err(err) => {
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(body::boxed(http_body::Full::from(err)))
                    .unwrap();
            }
        };

        let mut res = Response::new(body.unwrap_or_else(|| body::boxed(http_body::Empty::new())));
        if let Some(version) = version {
            *res.version_mut() = version;
        }
        *res.status_mut() = status;
        *res.headers_mut() = headers;
        *res.extensions_mut() = extensions;

        res
    }

    fn sealed(_: sealed::DontImplementThisTrait) {}
}

/// Parts of a response.
///
/// Used with [`IntoResponseParts`].
#[derive(Debug)]
pub struct ResponseParts {
    version: Option<Version>,
    status: StatusCode,
    headers: Result<HeaderMap, String>,
    extensions: Extensions,
    body: Option<BoxBody>,
}

impl ResponseParts {
    /// Set the HTTP version of the response.
    ///
    /// The default version is decided by hyper based on the HTTP version of the request.
    pub fn set_version(&mut self, version: Version) {
        self.version = Some(version);
    }

    /// Set the status code of the response.
    ///
    /// The default status code is `200 OK`.
    pub fn set_status(&mut self, status: StatusCode) {
        self.status = status;
    }

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
        if let Ok(headers) = &mut self.headers {
            let key = match key.try_into() {
                Ok(key) => key,
                Err(err) => {
                    self.headers = Err(err.to_string());
                    return;
                }
            };

            let value = match value.try_into() {
                Ok(value) => value,
                Err(err) => {
                    self.headers = Err(err.to_string());
                    return;
                }
            };

            f(headers, key, value);
        }
    }

    /// Insert an extension into the response.
    ///
    /// If the extension already exists it will be overwritten.
    pub fn insert_extension<T>(&mut self, extension: T)
    where
        T: Send + Sync + 'static,
    {
        self.extensions.insert(extension);
    }

    /// Set the body of the response.
    ///
    /// If the response already has a body it will be overwritten.
    pub fn set_body<B>(&mut self, body: B)
    where
        B: http_body::Body<Data = Bytes> + Send + 'static,
        B::Error: Into<BoxError>,
    {
        self.body = Some(body::boxed(body));
    }
}

impl Extend<(Option<HeaderName>, HeaderValue)> for ResponseParts {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (Option<HeaderName>, HeaderValue)>,
    {
        if let Ok(headers) = &mut self.headers {
            headers.extend(iter);
        }
    }
}

macro_rules! impl_into_response_parts {
    ( $($ty:ident),* $(,)? ) => {
        #[allow(non_snake_case)]
        impl<$($ty,)*> IntoResponseParts for ($($ty,)*)
        where
            $( $ty: IntoResponseParts, )*
        {
            fn into_response_parts(self, res: &mut ResponseParts) {
                let ($($ty,)*) = self;
                $( $ty.into_response_parts(res); )*

            }
        }
    };
}

all_the_tuples!(impl_into_response_parts);

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

impl IntoResponseParts for () {
    fn into_response_parts(self, _res: &mut ResponseParts) {}
}

impl IntoResponseParts for Infallible {
    fn into_response_parts(self, _res: &mut ResponseParts) {
        match self {}
    }
}

// `Result<T, E>` implements `IntoResponse` and not `IntoResponseParts` because otherwise
// `Result<impl IntoResponse, E>` wouldn't work.
//
// This means you cannot include results in tuples of parts, ie `(some_result, body)`. But
// thats probably fine.
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

    fn sealed(_: sealed::DontImplementThisTrait) {}
}

impl<B> IntoResponse for Response<B>
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    fn into_response(self) -> Response {
        self.map(body::boxed)
    }

    fn sealed(_: sealed::DontImplementThisTrait) {}
}

impl IntoResponse for http::response::Parts {
    fn into_response(self) -> Response {
        Response::from_parts(self, body::boxed(Empty::new()))
    }

    fn sealed(_: sealed::DontImplementThisTrait) {}
}

impl IntoResponseParts for Full<Bytes> {
    fn into_response_parts(self, res: &mut ResponseParts) {
        res.set_body(self);
    }
}

impl IntoResponseParts for Empty<Bytes> {
    fn into_response_parts(self, res: &mut ResponseParts) {
        res.set_body(self);
    }
}

impl<E> IntoResponseParts for http_body::combinators::BoxBody<Bytes, E>
where
    E: Into<BoxError> + 'static,
{
    fn into_response_parts(self, res: &mut ResponseParts) {
        res.set_body(self);
    }
}

impl<E> IntoResponseParts for http_body::combinators::UnsyncBoxBody<Bytes, E>
where
    E: Into<BoxError> + 'static,
{
    fn into_response_parts(self, res: &mut ResponseParts) {
        res.set_body(self);
    }
}

impl<B, F> IntoResponseParts for MapData<B, F>
where
    B: http_body::Body + Send + 'static,
    F: FnMut(B::Data) -> Bytes + Send + 'static,
    B::Error: Into<BoxError>,
{
    fn into_response_parts(self, res: &mut ResponseParts) {
        res.set_body(self);
    }
}

impl<B, F, E> IntoResponseParts for MapErr<B, F>
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    F: FnMut(B::Error) -> E + Send + 'static,
    E: Into<BoxError>,
{
    fn into_response_parts(self, res: &mut ResponseParts) {
        res.set_body(self);
    }
}

impl IntoResponseParts for &'static str {
    fn into_response_parts(self, res: &mut ResponseParts) {
        Cow::Borrowed(self).into_response_parts(res)
    }
}

impl IntoResponseParts for String {
    fn into_response_parts(self, res: &mut ResponseParts) {
        Cow::<'static, str>::Owned(self).into_response_parts(res)
    }
}

impl IntoResponseParts for Cow<'static, str> {
    fn into_response_parts(self, res: &mut ResponseParts) {
        res.set_body(Full::from(self));
        res.insert_header(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
        );
    }
}

impl IntoResponseParts for Bytes {
    fn into_response_parts(self, res: &mut ResponseParts) {
        res.set_body(Full::from(self));
        res.insert_header(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
    }
}

impl IntoResponseParts for BytesMut {
    fn into_response_parts(self, res: &mut ResponseParts) {
        self.freeze().into_response_parts(res)
    }
}

impl<T, U> IntoResponseParts for Chain<T, U>
where
    T: Buf + Unpin + Send + 'static,
    U: Buf + Unpin + Send + 'static,
{
    fn into_response_parts(self, res: &mut ResponseParts) {
        let (first, second) = self.into_inner();
        res.set_body(BytesChainBody {
            first: Some(first),
            second: Some(second),
        });
        res.insert_header(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
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

impl IntoResponseParts for &'static [u8] {
    fn into_response_parts(self, res: &mut ResponseParts) {
        Cow::Borrowed(self).into_response_parts(res)
    }
}

impl IntoResponseParts for Vec<u8> {
    fn into_response_parts(self, res: &mut ResponseParts) {
        Cow::<'static, [u8]>::Owned(self).into_response_parts(res)
    }
}

impl IntoResponseParts for Cow<'static, [u8]> {
    fn into_response_parts(self, res: &mut ResponseParts) {
        res.set_body(Full::from(self));
        res.insert_header(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
    }
}

impl IntoResponseParts for StatusCode {
    fn into_response_parts(self, res: &mut ResponseParts) {
        res.set_status(self);
    }
}

impl IntoResponseParts for HeaderMap {
    fn into_response_parts(self, res: &mut ResponseParts) {
        res.extend(self);
    }
}

impl IntoResponseParts for Version {
    fn into_response_parts(self, res: &mut ResponseParts) {
        res.set_version(self);
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[tokio::test]
    async fn building_response_from_tuples() {
        let mut headers = HeaderMap::new();
        headers.insert(header::SERVER, "axum".parse().unwrap());

        let res = (
            StatusCode::NOT_FOUND,
            [("x-foo", "foo")],
            headers,
            "body",
            Version::HTTP_2,
        )
            .into_response();

        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        assert_eq!(res.headers()["server"], "axum");
        assert_eq!(res.headers()["x-foo"], "foo");
        assert_eq!(res.version(), Version::HTTP_2);

        let body = crate::body::to_bytes(res).await.unwrap();
        assert_eq!(&body[..], b"body");
    }
}
