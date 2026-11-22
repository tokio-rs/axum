use axum_core::{
    body::Body,
    response::{IntoResponse, Response},
    Error,
};
use bytes::Bytes;
use http_body::Body as HttpBody;
use pin_project_lite::pin_project;
use std::{
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
    ///             (StatusCode::NOT_FOUND, format!("File not found: {err}"))
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
    #[must_use]
    pub struct AsyncReadBody {
        #[pin]
        body: Body,
    }
}

const DEFAULT_CHUNK_SIZE: usize = 64 * 1024;

impl AsyncReadBody {
    /// Create a new `AsyncReadBody`.
    ///
    /// The reader is read in chunks of up to 64 KiB. Use
    /// [`with_chunk_size`](Self::with_chunk_size) to change that.
    pub fn new<R>(read: R) -> Self
    where
        R: AsyncRead + Send + 'static,
    {
        Self::with_chunk_size(read, DEFAULT_CHUNK_SIZE)
    }

    /// Create a new `AsyncReadBody` that reads up to `chunk_size` bytes at a
    /// time.
    pub fn with_chunk_size<R>(read: R, chunk_size: usize) -> Self
    where
        R: AsyncRead + Send + 'static,
    {
        Self {
            body: Body::from_stream(ReaderStream::with_capacity(read, chunk_size)),
        }
    }
}

impl HttpBody for AsyncReadBody {
    type Data = Bytes;
    type Error = Error;

    #[inline]
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        self.project().body.poll_frame(cx)
    }

    #[inline]
    fn is_end_stream(&self) -> bool {
        self.body.is_end_stream()
    }

    #[inline]
    fn size_hint(&self) -> http_body::SizeHint {
        self.body.size_hint()
    }
}

impl IntoResponse for AsyncReadBody {
    fn into_response(self) -> Response {
        self.body.into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;
    use std::io::Cursor;

    async fn frame_sizes(mut body: AsyncReadBody) -> Vec<usize> {
        let mut sizes = Vec::new();
        while let Some(frame) = body.frame().await {
            sizes.push(frame.unwrap().into_data().unwrap().len());
        }
        sizes
    }

    #[tokio::test]
    async fn reads_in_large_chunks() {
        let data = vec![0; 2 * DEFAULT_CHUNK_SIZE + 10];

        let body = AsyncReadBody::new(Cursor::new(data.clone()));
        assert_eq!(
            frame_sizes(body).await,
            [DEFAULT_CHUNK_SIZE, DEFAULT_CHUNK_SIZE, 10]
        );

        let body = AsyncReadBody::with_chunk_size(Cursor::new(data), DEFAULT_CHUNK_SIZE + 10);
        assert_eq!(
            frame_sizes(body).await,
            [DEFAULT_CHUNK_SIZE + 10, DEFAULT_CHUNK_SIZE]
        );
    }
}
