use crate::{BoxError, Error};
use bytes::Bytes;
use futures_util::stream::{self, Stream, TryStreamExt};
use http::HeaderMap;
use http_body::Body;
use std::convert::Infallible;
use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};
use sync_wrapper::SyncWrapper;

/// An [`http_body::Body`] created from a [`Stream`].
///
/// # Example
///
/// ```
/// use axum::{
///     Router,
///     handler::get,
///     body::StreamBody,
/// };
/// use futures::stream;
///
/// async fn handler() -> StreamBody {
///     let chunks: Vec<Result<_, std::io::Error>> = vec![
///         Ok("Hello,"),
///         Ok(" "),
///         Ok("world!"),
///     ];
///     let stream = stream::iter(chunks);
///     StreamBody::new(stream)
/// }
///
/// let app = Router::new().route("/", get(handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// [`Stream`]: futures_util::stream::Stream
// this should probably be extracted to `http_body`, eventually...
pub struct StreamBody {
    stream: SyncWrapper<Pin<Box<dyn Stream<Item = Result<Bytes, Error>> + Send>>>,
}

impl StreamBody {
    /// Create a new `StreamBody` from a [`Stream`].
    ///
    /// [`Stream`]: futures_util::stream::Stream
    pub fn new<S, T, E>(stream: S) -> Self
    where
        S: Stream<Item = Result<T, E>> + Send + 'static,
        T: Into<Bytes> + 'static,
        E: Into<BoxError> + 'static,
    {
        let stream = stream
            .map_ok(Into::into)
            .map_err(|err| Error::new(err.into()));
        Self {
            stream: SyncWrapper::new(Box::pin(stream)),
        }
    }
}

impl Default for StreamBody {
    fn default() -> Self {
        Self::new(stream::empty::<Result<Bytes, Infallible>>())
    }
}

impl fmt::Debug for StreamBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StreamBody").finish()
    }
}

impl Body for StreamBody {
    type Data = Bytes;
    type Error = Error;

    fn poll_data(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        Pin::new(self.stream.get_mut()).poll_next(cx)
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }
}

#[test]
fn stream_body_traits() {
    crate::tests::assert_send::<StreamBody>();
    crate::tests::assert_sync::<StreamBody>();
    crate::tests::assert_unpin::<StreamBody>();
}
