use super::{IntoResponseParts, Response, ResponseParts};
use crate::{body, BoxError};
use bytes::{buf::Chain, Buf, Bytes, BytesMut};
use http::{
    header::{self, HeaderMap, HeaderValue},
    StatusCode, Version,
};
use http_body::{
    combinators::{MapData, MapErr},
    Empty, Full, SizeHint,
};
use std::{
    borrow::Cow,
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};

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
///             MyError::SomethingWentWrong => "something went wrong",
///             MyError::SomethingElseWentWrong => "something else went wrong",
///         };
///
///         // its often easier to implement `IntoResponse` by calling other implementations
///         (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
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

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        *res.status_mut() = self;
        res
    }
}

impl IntoResponse for Version {
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        *res.version_mut() = self;
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

impl<T> IntoResponse for T
where
    T: IntoResponseParts,
{
    fn into_response(self) -> Response {
        let res = ().into_response();
        let mut parts = ResponseParts { res: Ok(res) };

        self.into_response_parts(&mut parts);

        match parts.res {
            Ok(res) => res,
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
        }
    }
}

impl<R> IntoResponse for (StatusCode, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut res = self.1.into_response();
        *res.status_mut() = self.0;
        res
    }
}

impl<R> IntoResponse for (Version, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut res = self.1.into_response();
        *res.version_mut() = self.0;
        res
    }
}

impl<R> IntoResponse for (Version, StatusCode, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let (version, status, res) = self;
        let mut res = res.into_response();
        *res.version_mut() = version;
        *res.status_mut() = status;
        res
    }
}

macro_rules! impl_into_response {
    ( $($ty:ident),* $(,)? ) => {
        #[allow(non_snake_case)]
        impl<R, $($ty,)*> IntoResponse for ($($ty),*, R)
        where
            $( $ty: IntoResponseParts, )*
            R: IntoResponse,
        {
            fn into_response(self) -> Response {
                let ($($ty),*, res) = self;

                let res = res.into_response();
                let mut parts = ResponseParts { res: Ok(res) };

                $(
                    $ty.into_response_parts(&mut parts);
                )*

                match parts.res {
                    Ok(res) => res,
                    Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
                }
            }
        }

        #[allow(non_snake_case)]
        impl<R, $($ty,)*> IntoResponse for (StatusCode, $($ty),*, R)
        where
            $( $ty: IntoResponseParts, )*
            R: IntoResponse,
        {
            fn into_response(self) -> Response {
                let (status, $($ty),*, res) = self;

                let res = res.into_response();
                let mut parts = ResponseParts { res: Ok(res) };

                $(
                    $ty.into_response_parts(&mut parts);
                )*

                match parts.res {
                    Ok(mut res) => {
                        *res.status_mut() = status;
                        res
                    }
                    Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
                }
            }
        }

        #[allow(non_snake_case)]
        impl<R, $($ty,)*> IntoResponse for (Version, StatusCode, $($ty),*, R)
        where
            $( $ty: IntoResponseParts, )*
            R: IntoResponse,
        {
            fn into_response(self) -> Response {
                let (version, status, $($ty),*, res) = self;

                let res = res.into_response();
                let mut parts = ResponseParts { res: Ok(res) };

                $(
                    $ty.into_response_parts(&mut parts);
                )*

                match parts.res {
                    Ok(mut res) => {
                        *res.version_mut() = version;
                        *res.status_mut() = status;
                        res
                    }
                    Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
                }
            }
        }
    }
}

all_the_tuples!(impl_into_response);
