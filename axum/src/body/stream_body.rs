use crate::{
    body::{self, Bytes, HttpBody},
    response::{IntoResponse, Response},
    Error,
};
use futures_util::{
    ready,
    stream::{self, Stream},
};
use http::HeaderMap;
use pin_project_lite::pin_project;
use std::{
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
    /// # Example
    ///
    /// ```
    /// use axum::{
    ///     Router,
    ///     routing::get,
    ///     body::StreamBody,
    ///     response::IntoResponse,
    /// };
    /// use futures::stream::{self, Stream};
    /// use std::io;
    ///
    /// async fn handler() -> StreamBody<impl Stream<Item = &'static str>> {
    ///     let chunks: Vec<_> = vec![
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
    pub struct StreamBody<S> {
        #[pin]
        stream: SyncWrapper<S>,
    }
}

impl<S> From<S> for StreamBody<S>
where
    S: Stream + Send + 'static,
    S::Item: Into<Bytes>,
{
    fn from(stream: S) -> Self {
        Self::new(stream)
    }
}

impl<S> StreamBody<S> {
    /// Create a new `StreamBody` from a [`Stream`].
    pub fn new(stream: S) -> Self
    where
        S: Stream + Send + 'static,
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

impl Default for StreamBody<stream::Empty<Bytes>> {
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
    type Error = Error;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        let stream = self.project().stream.get_pin_mut();
        match ready!(stream.poll_next(cx)) {
            Some(chunk) => Poll::Ready(Some(Ok(chunk.into()))),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{body::StreamBody, routing::get, Router};
    use futures::stream::Stream;
    use futures_util::stream::Empty;
    use http::Request;
    use tower::ServiceExt;

    #[test]
    fn stream_body_traits() {
        type EmptyStream = StreamBody<Empty<Bytes>>;

        crate::test_helpers::assert_send::<EmptyStream>();
        crate::test_helpers::assert_sync::<EmptyStream>();
        crate::test_helpers::assert_unpin::<EmptyStream>();
    }

    #[tokio::test]
    async fn body_streaming_works() {
        async fn handler() -> StreamBody<impl Stream<Item = &'static str>> {
            let stream = futures::stream::iter(["foo", " ", "bar"]);
            StreamBody::new(stream)
        }

        let app = Router::new().route("/stream", get(handler));
        let resp = app
            .oneshot(Request::get("/stream").body(body::Body::empty()).unwrap())
            .await
            .unwrap();
        let body = hyper::body::to_bytes(resp).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(body, "foo bar")
    }
}
