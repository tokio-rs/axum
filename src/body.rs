use bytes::Buf;
use futures_util::ready;
use http_body::{Body as _, Empty};
use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

pub use hyper::body::Body;

use crate::BoxStdError;

/// A boxed [`Body`] trait object.
pub struct BoxBody<D, E> {
    inner: Pin<Box<dyn http_body::Body<Data = D, Error = E> + Send + Sync + 'static>>,
}

impl<D, E> BoxBody<D, E> {
    /// Create a new `BoxBody`.
    pub fn new<B>(body: B) -> Self
    where
        B: http_body::Body<Data = D, Error = E> + Send + Sync + 'static,
        D: Buf,
    {
        Self {
            inner: Box::pin(body),
        }
    }
}

// TODO: upstream this to http-body?
impl<D, E> Default for BoxBody<D, E>
where
    D: bytes::Buf + 'static,
{
    fn default() -> Self {
        BoxBody::new(Empty::<D>::new().map_err(|err| match err {}))
    }
}

impl<D, E> fmt::Debug for BoxBody<D, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxBody").finish()
    }
}

// when we've gotten rid of `BoxStdError` then we can remove this
impl<D, E> http_body::Body for BoxBody<D, E>
where
    D: Buf,
    E: Into<tower::BoxError>,
{
    type Data = D;
    type Error = BoxStdError;

    fn poll_data(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        match ready!(self.inner.as_mut().poll_data(cx)) {
            Some(Ok(chunk)) => Some(Ok(chunk)).into(),
            Some(Err(err)) => Some(Err(BoxStdError(err.into()))).into(),
            None => None.into(),
        }
    }

    fn poll_trailers(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        match ready!(self.inner.as_mut().poll_trailers(cx)) {
            Ok(trailers) => Ok(trailers).into(),
            Err(err) => Err(BoxStdError(err.into())).into(),
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

impl From<String> for BoxBody<bytes::Bytes, tower::BoxError> {
    fn from(s: String) -> Self {
        let body = hyper::Body::from(s);
        let body = body.map_err(Into::<tower::BoxError>::into);

        BoxBody {
            inner: Box::pin(body),
        }
    }
}
