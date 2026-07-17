use bytes::Buf;
use http_body::{Body, Frame, SizeHint};
use std::{
    convert::Infallible,
    fmt,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

/// Refer to the documentation of [`super::Body::unknown`] which is `pub`.
pub(crate) struct Unknown<D> {
    _marker: PhantomData<fn() -> D>,
}

impl<D> Unknown<D> {
    pub(crate) const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<D: Buf> Body for Unknown<D> {
    type Data = D;
    type Error = Infallible;

    #[inline]
    fn poll_frame(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Poll::Ready(None)
    }

    fn is_end_stream(&self) -> bool {
        true
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}

impl<D> fmt::Debug for Unknown<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Unknown").finish()
    }
}

impl<D> Default for Unknown<D> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D> Clone for Unknown<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for Unknown<D> {}
