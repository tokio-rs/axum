//! Streaming bodies.

use crate::{
    body::{self, Bytes, HttpBody},
    response::{IntoResponse, Response},
    Error,
};
use futures_util::{
    ready,
    stream::{self, Stream, TryStream},
};
use http::HeaderMap;
use pin_project_lite::pin_project;
use std::{
    convert::Infallible,
    fmt,
    pin::Pin,
    task::{Context, Poll},
};
use sync_wrapper::SyncWrapper;

pin_project! {
    /// An [`http_body::Body`] created from a [`Stream`].
    ///
    /// The purpose of this type is to be used in responses. If you want to
    /// extract the request body as a stream consider using
    /// [`BodyStream`](crate::extract::BodyStream).
    ///
    /// Note the inner stream must yield `impl Into<Bytes>`. If you need `Result` use
    /// [`TryStreamBody`] instead.
    ///
    /// # Example
    ///
    /// ```
    /// use axum::{
    ///     Router,
    ///     routing::get,
    ///     body::StreamBody,
    ///     response::{IntoResponse, Response},
    /// };
    /// use futures::stream::{self, Stream};
    ///
    /// async fn handler() -> Response {
    ///     let chunks = Vec::from(["Hello,", " ", "world!"]);
    ///     let stream = stream::iter(chunks);
    ///     StreamBody::new(stream).into_response()
    /// }
    ///
    /// let app = Router::new().route("/", get(handler));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// [`Stream`]: futures_util::stream::Stream
    pub struct StreamBody<S> {
        #[pin]
        stream: SyncWrapper<S>,
    }
}

impl<S> From<S> for StreamBody<S>
where
    S: Stream,
    S::Item: Into<Bytes>,
{
    fn from(stream: S) -> Self {
        Self::new(stream)
    }
}

impl<S> StreamBody<S> {
    /// Create a new `StreamBody` from a [`Stream`].
    ///
    /// [`Stream`]: futures_util::stream::Stream
    pub fn new(stream: S) -> Self
    where
        S: Stream,
        S::Item: Into<Bytes>,
    {
        Self {
            stream: SyncWrapper::new(stream),
        }
    }
}

impl<S> IntoResponse for StreamBody<S>
where
    S: Stream + Send + 'static,
    S::Item: Into<Bytes>,
{
    fn into_response(self) -> Response {
        Response::new(body::boxed(self))
    }
}

impl Default for StreamBody<futures_util::stream::Empty<Bytes>> {
    fn default() -> Self {
        Self::new(stream::empty())
    }
}

impl<S> fmt::Debug for StreamBody<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StreamBody").finish()
    }
}

impl<S> HttpBody for StreamBody<S>
where
    S: Stream,
    S::Item: Into<Bytes>,
{
    type Data = Bytes;
    type Error = Infallible;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        self.poll_next(cx).map(|option_chunk| option_chunk.map(Ok))
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }
}

impl<S> Stream for StreamBody<S>
where
    S: Stream,
    S::Item: Into<Bytes>,
{
    type Item = Bytes;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project()
            .stream
            .get_pin_mut()
            .poll_next(cx)
            .map(|option_chunk| option_chunk.map(Into::into))
    }
}

pin_project! {
    /// An [`http_body::Body`] created from a fallible [`Stream`].
    ///
    /// The purpose of this type is to be used in responses. If you want to
    /// extract the request body as a stream consider using
    /// [`BodyStream`](crate::extract::BodyStream).
    ///
    /// # Example
    ///
    /// ```
    /// use axum::{
    ///     Router,
    ///     routing::get,
    ///     body::{TryStreamBody, Bytes},
    ///     response::{IntoResponse, Response},
    /// };
    /// use futures::stream::{self, Stream};
    /// use std::io;
    ///
    /// async fn handler() -> Response {
    ///     TryStreamBody::new(some_fallible_stream())
    ///         .on_error(on_stream_error)
    ///         .into_response()
    /// }
    ///
    /// fn some_fallible_stream() -> impl Stream<Item = Result<Bytes, io::Error>> + Send + 'static {
    ///     // ...
    ///     # futures_util::stream::empty()
    /// }
    ///
    /// fn on_stream_error(err: io::Error) {
    ///     // ...
    /// }
    ///
    /// let app = Router::new().route("/", get(handler));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// [`Stream`]: futures_util::stream::Stream
    pub struct TryStreamBody<S, T = DefaultOnError> {
        #[pin]
        stream: SyncWrapper<S>,
        on_error: T,
    }
}

impl<S> From<S> for TryStreamBody<S, DefaultOnError>
where
    S: TryStream,
    S::Ok: Into<Bytes>,
{
    fn from(stream: S) -> Self {
        Self::new(stream)
    }
}

impl<S> TryStreamBody<S, DefaultOnError> {
    /// Create a new `TryStreamBody` from a [`Stream`].
    ///
    /// [`Stream`]: futures_util::stream::Stream
    pub fn new(stream: S) -> Self
    where
        S: TryStream,
        S::Ok: Into<Bytes>,
    {
        Self {
            stream: SyncWrapper::new(stream),
            on_error: DefaultOnError,
        }
    }
}

impl<S, T> TryStreamBody<S, T> {
    /// Provide a callback to call if the underlying stream produces an error.
    ///
    /// By default any errors will be silently ignored.
    pub fn on_error<C>(self, callback: C) -> TryStreamBody<S, C>
    where
        S: TryStream,
        C: OnError<S::Error>,
    {
        TryStreamBody {
            stream: self.stream,
            on_error: callback,
        }
    }
}

impl<S, T> IntoResponse for TryStreamBody<S, T>
where
    S: TryStream + Send + 'static,
    S::Ok: Into<Bytes>,
    T: OnError<S::Error> + Send + 'static,
{
    fn into_response(self) -> Response {
        Response::new(body::boxed(self))
    }
}

impl Default for TryStreamBody<futures_util::stream::Empty<Result<Bytes, Error>>, DefaultOnError> {
    fn default() -> Self {
        Self::new(stream::empty())
    }
}

impl<S, T> fmt::Debug for TryStreamBody<S, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryStreamBody").finish_non_exhaustive()
    }
}

impl<S, T> HttpBody for TryStreamBody<S, T>
where
    S: TryStream,
    S::Ok: Into<Bytes>,
    T: OnError<S::Error>,
{
    type Data = Bytes;
    type Error = Infallible;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        let this = self.project();
        let stream = this.stream.get_pin_mut();
        match ready!(stream.try_poll_next(cx)) {
            Some(Ok(chunk)) => Poll::Ready(Some(Ok(chunk.into()))),
            Some(Err(err)) => {
                this.on_error.call(err);
                Poll::Ready(None)
            }
            None => Poll::Ready(None),
        }
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }
}

impl<S, T> Stream for TryStreamBody<S, T>
where
    S: TryStream,
    S::Ok: Into<Bytes>,
    T: OnError<S::Error>,
{
    type Item = Bytes;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let stream = this.stream.get_pin_mut();
        match ready!(stream.try_poll_next(cx)) {
            Some(Ok(chunk)) => Poll::Ready(Some(chunk.into())),
            Some(Err(err)) => {
                this.on_error.call(err);
                Poll::Ready(None)
            }
            None => Poll::Ready(None),
        }
    }
}

/// What to do when a stream produces an error.
///
/// See [`TryStreamBody`] for more details.
pub trait OnError<E> {
    /// Call the callback.
    fn call(&mut self, error: E);
}

/// The default `OnError` used by `TryStreamBody`.
///
/// It simply ignores the error.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultOnError;

impl<E> OnError<E> for DefaultOnError {
    fn call(&mut self, _error: E) {}
}

impl<E, F> OnError<E> for F
where
    F: FnMut(E) + Send + 'static,
{
    fn call(&mut self, error: E) {
        self(error)
    }
}

#[test]
fn stream_body_traits() {
    use futures_util::stream::Empty;

    type EmptyStream = StreamBody<Empty<Result<Bytes, crate::BoxError>>>;

    crate::test_helpers::assert_send::<EmptyStream>();
    crate::test_helpers::assert_sync::<EmptyStream>();
    crate::test_helpers::assert_unpin::<EmptyStream>();
}
