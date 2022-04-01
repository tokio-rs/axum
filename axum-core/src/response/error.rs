use super::{IntoResponse, Response};
use crate::BoxError;

use bytes::{buf::Chain, Buf, Bytes, BytesMut};
use http::header::{HeaderMap, HeaderName, HeaderValue};
use http::StatusCode;
use http_body::combinators::{MapData, MapErr};
use http_body::{Empty, Full};
use std::borrow::Cow;
use std::convert::{Infallible, TryInto};

/// An [IntoResponse]-based error type
///
/// All types which implement [IntoResponse] can be converted to an [Error].
/// This makes it useful as a general error type for functions which combine
/// multiple distinct error types but all of which implement [IntoResponse].
#[derive(Debug)]
pub struct Error(Response);

impl IntoResponse for Error {
    #[inline]
    fn into_response(self) -> Response {
        self.0
    }
}

impl From<StatusCode> for Error {
    fn from(value: StatusCode) -> Self {
        Self(value.into_response())
    }
}

impl From<()> for Error {
    fn from(value: ()) -> Self {
        Self(value.into_response())
    }
}

impl From<Infallible> for Error {
    fn from(value: Infallible) -> Self {
        Self(value.into_response())
    }
}

impl<T, E> From<Result<T, E>> for Error
where
    T: IntoResponse,
    E: IntoResponse,
{
    fn from(value: Result<T, E>) -> Self {
        Self(value.into_response())
    }
}

impl<B> From<Response<B>> for Error
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    fn from(value: Response<B>) -> Self {
        Self(value.into_response())
    }
}

impl From<http::response::Parts> for Error {
    fn from(value: http::response::Parts) -> Self {
        Self(value.into_response())
    }
}

impl From<Full<Bytes>> for Error {
    fn from(value: Full<Bytes>) -> Self {
        Self(value.into_response())
    }
}

impl From<Empty<Bytes>> for Error {
    fn from(value: Empty<Bytes>) -> Self {
        Self(value.into_response())
    }
}

impl<E> From<http_body::combinators::BoxBody<Bytes, E>> for Error
where
    E: Into<BoxError> + 'static,
{
    fn from(value: http_body::combinators::BoxBody<Bytes, E>) -> Self {
        Self(value.into_response())
    }
}

impl<E> From<http_body::combinators::UnsyncBoxBody<Bytes, E>> for Error
where
    E: Into<BoxError> + 'static,
{
    fn from(value: http_body::combinators::UnsyncBoxBody<Bytes, E>) -> Self {
        Self(value.into_response())
    }
}

impl<B, F> From<MapData<B, F>> for Error
where
    B: http_body::Body + Send + 'static,
    F: FnMut(B::Data) -> Bytes + Send + 'static,
    B::Error: Into<BoxError>,
{
    fn from(value: MapData<B, F>) -> Self {
        Self(value.into_response())
    }
}

impl<B, F, E> From<MapErr<B, F>> for Error
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    F: FnMut(B::Error) -> E + Send + 'static,
    E: Into<BoxError>,
{
    fn from(value: MapErr<B, F>) -> Self {
        Self(value.into_response())
    }
}

impl From<&'static str> for Error {
    fn from(value: &'static str) -> Self {
        Self(value.into_response())
    }
}

impl From<String> for Error {
    fn from(value: String) -> Self {
        Self(value.into_response())
    }
}

impl From<Cow<'static, str>> for Error {
    fn from(value: Cow<'static, str>) -> Self {
        Self(value.into_response())
    }
}

impl From<Bytes> for Error {
    fn from(value: Bytes) -> Self {
        Self(value.into_response())
    }
}

impl From<BytesMut> for Error {
    fn from(value: BytesMut) -> Self {
        Self(value.into_response())
    }
}

impl<T, U> From<Chain<T, U>> for Error
where
    T: Buf + Unpin + Send + 'static,
    U: Buf + Unpin + Send + 'static,
{
    fn from(value: Chain<T, U>) -> Self {
        Self(value.into_response())
    }
}

impl From<&'static [u8]> for Error {
    fn from(value: &'static [u8]) -> Self {
        Self(value.into_response())
    }
}

impl From<Vec<u8>> for Error {
    fn from(value: Vec<u8>) -> Self {
        Self(value.into_response())
    }
}

impl From<Cow<'static, [u8]>> for Error {
    fn from(value: Cow<'static, [u8]>) -> Self {
        Self(value.into_response())
    }
}

impl<R> From<(StatusCode, R)> for Error
where
    R: IntoResponse,
{
    fn from(value: (StatusCode, R)) -> Self {
        Self(value.into_response())
    }
}

impl From<HeaderMap> for Error {
    fn from(value: HeaderMap) -> Self {
        Self(value.into_response())
    }
}

impl<K, V, const N: usize> From<[(K, V); N]> for Error
where
    K: TryInto<HeaderName>,
    K::Error: std::fmt::Display,
    V: TryInto<HeaderValue>,
    V::Error: std::fmt::Display,
{
    fn from(value: [(K, V); N]) -> Self {
        Self(value.into_response())
    }
}

macro_rules! impl_into_response {
    ( $($ty:ident),* $(,)? ) => {
        impl<R, $($ty,)*> From<($($ty),*, R)> for Error
        where
            $( $ty: $crate::response::IntoResponseParts ),*,
            R: IntoResponse,
        {
            fn from(value: ($($ty),*, R)) -> Self {
                Self(value.into_response())
            }
        }
    }
}

all_the_tuples!(impl_into_response);
