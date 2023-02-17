use axum::{
    body::{
        self,
        stream_body::{DefaultOnError, OnError, TryStreamBody},
        Bytes, HttpBody,
    },
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use pin_project_lite::pin_project;
use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::AsyncRead;
use tokio_util::io::ReaderStream;

pin_project! {
    /// An [`HttpBody`] created from an [`AsyncRead`].
    ///
    /// # Example
    ///
    /// `AsyncReadBody` can be used to stream the contents of a file:
    ///
    /// ```rust
    /// use axum::{
    ///     Router,
    ///     routing::get,
    ///     http::{StatusCode, header::CONTENT_TYPE},
    ///     response::{Response, IntoResponse},
    /// };
    /// use axum_extra::body::AsyncReadBody;
    /// use tokio::fs::File;
    ///
    /// async fn cargo_toml() -> Result<Response, (StatusCode, String)> {
    ///     let file = File::open("Cargo.toml")
    ///         .await
    ///         .map_err(|err| {
    ///             (StatusCode::NOT_FOUND, format!("File not found: {}", err))
    ///         })?;
    ///
    ///     let headers = [(CONTENT_TYPE, "text/x-toml")];
    ///     let body = AsyncReadBody::new(file);
    ///     Ok((headers, body).into_response())
    /// }
    ///
    /// let app = Router::new().route("/Cargo.toml", get(cargo_toml));
    /// # let _: Router = app;
    /// ```
    #[cfg(feature = "async-read-body")]
    #[derive(Debug)]
    pub struct AsyncReadBody<R, T = DefaultOnError> {
        #[pin]
        read: TryStreamBody<ReaderStream<R>, T>,
    }
}

impl<R> AsyncReadBody<R, DefaultOnError> {
    /// Create a new `AsyncReadBody`.
    pub fn new(read: R) -> Self
    where
        R: AsyncRead,
    {
        Self {
            read: TryStreamBody::new(ReaderStream::new(read)),
        }
    }
}

impl<R, T> AsyncReadBody<R, T> {
    /// Provide a callback to call if the underlying reader produces an error.
    ///
    /// By default any errors will be silently ignored.
    pub fn on_error<C>(self, callback: C) -> AsyncReadBody<R, C>
    where
        R: AsyncRead,
        C: OnError<std::io::Error>,
    {
        AsyncReadBody {
            read: self.read.on_error(callback),
        }
    }
}

impl<R, T> HttpBody for AsyncReadBody<R, T>
where
    R: AsyncRead,
    T: OnError<std::io::Error>,
{
    type Data = Bytes;
    type Error = Infallible;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        self.project().read.poll_data(cx)
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }
}

impl<R, T> IntoResponse for AsyncReadBody<R, T>
where
    R: AsyncRead + Send + 'static,
    T: OnError<std::io::Error> + Send + 'static,
{
    fn into_response(self) -> Response {
        Response::new(body::boxed(self))
    }
}
