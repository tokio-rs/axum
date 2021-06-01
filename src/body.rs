use bytes::Bytes;
use http_body::{Empty, Full};
use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};
use tower::BoxError;

pub use hyper::body::Body;

use crate::BoxStdError;

/// A boxed [`Body`] trait object.
pub struct BoxBody {
    // when we've gotten rid of `BoxStdError` we should be able to change the error type to
    // `BoxError`
    inner: Pin<Box<dyn http_body::Body<Data = Bytes, Error = BoxStdError> + Send + Sync + 'static>>,
}

impl BoxBody {
    /// Create a new `BoxBody`.
    pub fn new<B>(body: B) -> Self
    where
        B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        B::Error: Into<BoxError> + Send + Sync + 'static,
    {
        Self {
            inner: Box::pin(body.map_err(|error| BoxStdError(error.into()))),
        }
    }
}

impl Default for BoxBody {
    fn default() -> Self {
        BoxBody::new(Empty::<Bytes>::new())
    }
}

impl fmt::Debug for BoxBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxBody").finish()
    }
}

impl http_body::Body for BoxBody {
    type Data = Bytes;
    type Error = BoxStdError;

    fn poll_data(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        self.inner.as_mut().poll_data(cx)
    }

    fn poll_trailers(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        self.inner.as_mut().poll_trailers(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

impl<B> From<B> for BoxBody
where
    B: Into<Bytes>,
{
    fn from(s: B) -> Self {
        BoxBody::new(Full::from(s.into()))
    }
}
