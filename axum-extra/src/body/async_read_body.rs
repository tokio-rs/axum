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

impl AsyncReadBody {
    /// Create a new `AsyncReadBody`.
    pub fn new<R>(read: R) -> Self
    where
        R: AsyncRead + Send + 'static,
    {
        Self {
            body: Body::from_stream(ReaderStream::new(read)),
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
