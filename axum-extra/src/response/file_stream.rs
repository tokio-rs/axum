use axum::{
    body,
    response::{IntoResponse, Response},
    BoxError,
};
use bytes::Bytes;
use futures_util::TryStream;
use http::{header, StatusCode};
use std::{io, path::PathBuf};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

/// Alias for `tokio_util::io::ReaderStream<File>`.
pub type AsyncReaderStream = ReaderStream<File>;

/// Encapsulate the file stream.
/// The encapsulated file stream construct requires passing in a stream
/// # Examples
///
/// ```
/// use axum::{
///     http::StatusCode,
///     response::{Response, IntoResponse},
///     Router,
///     routing::get
/// };
/// use axum_extra::response::file_stream::FileStream;
/// use tokio::fs::File;
/// use tokio_util::io::ReaderStream;
/// async fn file_stream() -> Result<Response, (StatusCode, String)> {
///     let stream=ReaderStream::new(File::open("test.txt").await.map_err(|e| (StatusCode::NOT_FOUND, format!("File not found: {e}")))?);
///     let file_stream_resp = FileStream::new(stream)
///         .file_name("test.txt");
//
///     Ok(file_stream_resp.into_response())
/// }
/// let app = Router::new().route("/FileStreamDownload", get(file_stream));
/// # let _: Router = app;
/// ```
#[derive(Debug)]
pub struct FileStream<S>
where
    S: TryStream + Send + 'static,
    S::Ok: Into<Bytes>,
    S::Error: Into<BoxError>,
{
    /// stream.
    pub stream: S,
    /// The file name of the file.
    pub file_name: Option<String>,
    /// The size of the file.
    pub content_size: Option<u64>,
}

impl<S> FileStream<S>
where
    S: TryStream + Send + 'static,
    S::Ok: Into<Bytes>,
    S::Error: Into<BoxError>,
{
    /// Create a file stream.
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            file_name: None,
            content_size: None,
        }
    }

    /// Create a file stream from a file path.
    /// # Examples
    /// ```
    /// use axum::{
    ///     http::StatusCode,
    ///     response::{Response, IntoResponse},
    ///     Router,
    ///     routing::get
    /// };
    /// use axum_extra::response::file_stream::FileStream;
    /// use std::path::PathBuf;
    /// use tokio::fs::File;
    /// use tokio_util::io::ReaderStream;
    /// async fn file_stream() -> Response {
    ///     FileStream::<ReaderStream<File>>::from_path(PathBuf::from("test.txt"))
    ///     .await
    ///     .map_err(|e| (StatusCode::NOT_FOUND, format!("File not found: {e}")))
    ///     .into_response()
    /// }
    /// let app = Router::new().route("/FileStreamDownload", get(file_stream));
    /// # let _: Router = app;
    /// ```
    pub async fn from_path(path: PathBuf) -> io::Result<FileStream<AsyncReaderStream>> {
        // open file
        let file = File::open(&path).await?;
        let mut content_size = None;
        let mut file_name = None;

        // get file metadata length
        if let Ok(metadata) = file.metadata().await {
            content_size = Some(metadata.len());
        }

        // get file name
        if let Some(file_name_os) = path.file_name() {
            if let Some(file_name_str) = file_name_os.to_str() {
                file_name = Some(file_name_str.to_owned());
            }
        }

        // return FileStream
        Ok(FileStream {
            stream: ReaderStream::new(file),
            file_name,
            content_size,
        })
    }

    /// Set the file name of the file.
    pub fn file_name<T: Into<String>>(mut self, file_name: T) -> Self {
        self.file_name = Some(file_name.into());
        self
    }

    /// Set the size of the file.
    pub fn content_size<T: Into<u64>>(mut self, len: T) -> Self {
        self.content_size = Some(len.into());
        self
    }
}

impl<S> IntoResponse for FileStream<S>
where
    S: TryStream + Send + 'static,
    S::Ok: Into<Bytes>,
    S::Error: Into<BoxError>,
{
    fn into_response(self) -> Response {
        let mut resp = Response::builder().header(header::CONTENT_TYPE, "application/octet-stream");

        if let Some(file_name) = self.file_name {
            resp = resp.header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", file_name),
            );
        };

        if let Some(content_size) = self.content_size {
            resp = resp.header(header::CONTENT_LENGTH, content_size);
        };

        resp.body(body::Body::from_stream(self.stream))
            .unwrap_or_else(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("build FileStream responsec error:{}", e),
                )
                    .into_response()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::Request, routing::get, Router};
    use body::Body;
    use http_body_util::BodyExt;
    use std::io::Cursor;
    use tokio_util::io::ReaderStream;
    use tower::ServiceExt;

    #[tokio::test]
    async fn response() -> Result<(), Box<dyn std::error::Error>> {
        let app = Router::new().route(
            "/file",
            get(|| async {
                // Simulating a file stream
                let file_content = b"Hello, this is the simulated file content!".to_vec();
                let reader = Cursor::new(file_content);

                // Response file stream
                // Content size and file name are not attached by default
                let stream = ReaderStream::new(reader);
                FileStream::new(stream).into_response()
            }),
        );

        // Simulating a GET request
        let response = app
            .oneshot(Request::builder().uri("/file").body(Body::empty())?)
            .await?;

        // Validate Response Status Code
        assert_eq!(response.status(), StatusCode::OK);

        // Validate Response Headers
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/octet-stream"
        );

        // Validate Response Body
        let body: &[u8] = &response.into_body().collect().await?.to_bytes();
        assert_eq!(
            std::str::from_utf8(body)?,
            "Hello, this is the simulated file content!"
        );
        Ok(())
    }

    #[tokio::test]
    async fn response_not_set_filename() -> Result<(), Box<dyn std::error::Error>> {
        let app = Router::new().route(
            "/file",
            get(|| async {
                // Simulating a file stream
                let file_content = b"Hello, this is the simulated file content!".to_vec();
                let size = file_content.len() as u64;
                let reader = Cursor::new(file_content);

                // Response file stream
                let stream = ReaderStream::new(reader);
                FileStream::new(stream).content_size(size).into_response()
            }),
        );

        // Simulating a GET request
        let response = app
            .oneshot(Request::builder().uri("/file").body(Body::empty())?)
            .await?;

        // Validate Response Status Code
        assert_eq!(response.status(), StatusCode::OK);

        // Validate Response Headers
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/octet-stream"
        );
        assert_eq!(response.headers().get("content-length").unwrap(), "42");

        // Validate Response Body
        let body: &[u8] = &response.into_body().collect().await?.to_bytes();
        assert_eq!(
            std::str::from_utf8(body)?,
            "Hello, this is the simulated file content!"
        );
        Ok(())
    }

    #[tokio::test]
    async fn response_not_set_content_size() -> Result<(), Box<dyn std::error::Error>> {
        let app = Router::new().route(
            "/file",
            get(|| async {
                // Simulating a file stream
                let file_content = b"Hello, this is the simulated file content!".to_vec();
                let reader = Cursor::new(file_content);

                // Response file stream
                let stream = ReaderStream::new(reader);
                FileStream::new(stream).file_name("test").into_response()
            }),
        );

        // Simulating a GET request
        let response = app
            .oneshot(Request::builder().uri("/file").body(Body::empty())?)
            .await?;

        // Validate Response Status Code
        assert_eq!(response.status(), StatusCode::OK);

        // Validate Response Headers
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/octet-stream"
        );
        assert_eq!(
            response.headers().get("content-disposition").unwrap(),
            "attachment; filename=\"test\""
        );

        // Validate Response Body
        let body: &[u8] = &response.into_body().collect().await?.to_bytes();
        assert_eq!(
            std::str::from_utf8(body)?,
            "Hello, this is the simulated file content!"
        );
        Ok(())
    }

    #[tokio::test]
    async fn response_with_content_size_and_filename() -> Result<(), Box<dyn std::error::Error>> {
        let app = Router::new().route(
            "/file",
            get(|| async {
                // Simulating a file stream
                let file_content = b"Hello, this is the simulated file content!".to_vec();
                let size = file_content.len() as u64;
                let reader = Cursor::new(file_content);

                // Response file stream
                let stream = ReaderStream::new(reader);
                FileStream::new(stream)
                    .file_name("test")
                    .content_size(size)
                    .into_response()
            }),
        );

        // Simulating a GET request
        let response = app
            .oneshot(Request::builder().uri("/file").body(Body::empty())?)
            .await?;

        // Validate Response Status Code
        assert_eq!(response.status(), StatusCode::OK);

        // Validate Response Headers
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/octet-stream"
        );
        assert_eq!(
            response.headers().get("content-disposition").unwrap(),
            "attachment; filename=\"test\""
        );
        assert_eq!(response.headers().get("content-length").unwrap(), "42");

        // Validate Response Body
        let body: &[u8] = &response.into_body().collect().await?.to_bytes();
        assert_eq!(
            std::str::from_utf8(body)?,
            "Hello, this is the simulated file content!"
        );
        Ok(())
    }

    #[tokio::test]
    async fn response_from_path() -> Result<(), Box<dyn std::error::Error>> {
        let app = Router::new().route(
            "/from_path",
            get(move || async move {
                FileStream::<AsyncReaderStream>::from_path("CHANGELOG.md".into())
                    .await
                    .unwrap()
                    .into_response()
            }),
        );

        // Simulating a GET request
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/from_path")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Validate Response Status Code
        assert_eq!(response.status(), StatusCode::OK);

        // Validate Response Headers
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/octet-stream"
        );
        assert_eq!(
            response.headers().get("content-disposition").unwrap(),
            "attachment; filename=\"CHANGELOG.md\""
        );

        let file = File::open("CHANGELOG.md").await.unwrap();
        // get file size
        let content_length = file.metadata().await.unwrap().len();

        assert_eq!(
            response
                .headers()
                .get("content-length")
                .unwrap()
                .to_str()
                .unwrap(),
            content_length.to_string()
        );
        Ok(())
    }
}
