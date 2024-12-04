use axum::{
    body,
    response::{IntoResponse, Response},
    BoxError,
};
use bytes::Bytes;
use futures_util::TryStream;
use http::{header, StatusCode};
use std::{io, path::Path};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt},
};
use tokio_util::io::ReaderStream;

/// Encapsulate the file stream.
///
/// The encapsulated file stream construct requires passing in a stream.
///
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
///
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
pub struct FileStream<S> {
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
    ///
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
    ///
    /// async fn file_stream() -> Response {
    ///     FileStream::<ReaderStream<File>>::from_path("test.txt")
    ///         .await
    ///         .map_err(|e| (StatusCode::NOT_FOUND, format!("File not found: {e}")))
    ///         .into_response()
    /// }
    /// let app = Router::new().route("/FileStreamDownload", get(file_stream));
    /// # let _: Router = app;
    /// ```
    pub async fn from_path(path: impl AsRef<Path>) -> io::Result<FileStream<ReaderStream<File>>> {
        // open file
        let file = File::open(&path).await?;
        let mut content_size = None;
        let mut file_name = None;

        // get file metadata length
        if let Ok(metadata) = file.metadata().await {
            content_size = Some(metadata.len());
        }

        // get file name
        if let Some(file_name_os) = path.as_ref().file_name() {
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
    pub fn file_name(mut self, file_name: impl Into<String>) -> Self {
        self.file_name = Some(file_name.into());
        self
    }

    /// Set the size of the file.
    pub fn content_size(mut self, len: u64) -> Self {
        self.content_size = Some(len);
        self
    }

    /// Return a range response.
    ///
    /// range: (start, end, total_size)
    ///
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
    /// use tokio::io::AsyncSeekExt;
    ///
    /// async fn range_response() -> Result<Response, (StatusCode, String)> {
    ///     let mut file=File::open("test.txt").await.map_err(|e| (StatusCode::NOT_FOUND, format!("File not found: {e}")))?;
    ///     let mut file_size=file.metadata().await.map_err(|e| (StatusCode::NOT_FOUND, format!("Get file size: {e}")))?.len();
    ///     file.seek(std::io::SeekFrom::Start(10)).await.map_err(|e| (StatusCode::NOT_FOUND, format!("File seek error: {e}")))?;
    ///     let stream=ReaderStream::new(file);
    ///  
    ///     Ok(FileStream::new(stream).into_range_response(10, file_size-1, file_size))
    /// }
    /// let app = Router::new().route("/FileStreamRange", get(range_response));
    /// # let _: Router = app;
    /// ```
    pub fn into_range_response(self, start: u64, end: u64, total_size: u64) -> Response {
        let mut resp = Response::builder().header(header::CONTENT_TYPE, "application/octet-stream");
        resp = resp.status(StatusCode::PARTIAL_CONTENT);

        resp = resp.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total_size}"),
        );

        resp.body(body::Body::from_stream(self.stream))
            .unwrap_or_else(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("build FileStream responsec error: {e}"),
                )
                    .into_response()
            })
    }

    /// Attempts to return RANGE requests directly from the file path.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The path of the file to be streamed
    /// * `start` - The start position of the range, if start > file size or start > end return Range Not Satisfiable
    /// * `end` - The end position of the range if end == 0 end = file size - 1
    ///
    /// # Examples
    ///
    /// ```
    /// use axum::{
    ///     http::StatusCode,
    ///     response::{Response, IntoResponse},
    ///     Router,
    ///     routing::get
    /// };
    /// use std::path::Path;
    /// use axum_extra::response::file_stream::FileStream;
    /// use tokio::fs::File;
    /// use tokio_util::io::ReaderStream;
    /// use tokio::io::AsyncSeekExt;
    ///
    /// async fn range_stream() -> Response {
    ///     let range_start = 0;
    ///     let range_end = 1024;
    ///
    ///     FileStream::<ReaderStream<File>>::try_range_response("CHANGELOG.md", range_start, range_end).await
    ///     .map_err(|e| (StatusCode::NOT_FOUND, format!("File not found: {e}")))
    ///     .into_response()
    ///     
    /// }
    /// let app = Router::new().route("/FileStreamRange", get(range_stream));
    /// # let _: Router = app;
    /// ```
    pub async fn try_range_response(
        file_path: impl AsRef<Path>,
        start: u64,
        mut end: u64,
    ) -> io::Result<Response> {
        // open file
        let mut file = File::open(file_path).await?;

        // get file metadata
        let metadata = file.metadata().await?;
        let total_size = metadata.len();

        if end == 0 {
            end = total_size - 1;
        }

        // range check
        if start > total_size {
            return Ok((StatusCode::RANGE_NOT_SATISFIABLE, "Range Not Satisfiable").into_response());
        }
        if start > end {
            return Ok((StatusCode::RANGE_NOT_SATISFIABLE, "Range Not Satisfiable").into_response());
        }
        if end >= total_size {
            return Ok((StatusCode::RANGE_NOT_SATISFIABLE, "Range Not Satisfiable").into_response());
        }

        // get file stream and seek to start to return range response
        file.seek(std::io::SeekFrom::Start(start)).await?;

        let stream = ReaderStream::new(file.take(end - start + 1));

        Ok(FileStream::new(stream).into_range_response(start, end, total_size))
    }
}

/// default  response is application/octet-stream and attachment mode;
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
                format!("attachment; filename=\"{file_name}\""),
            );
        }

        if let Some(content_size) = self.content_size {
            resp = resp.header(header::CONTENT_LENGTH, content_size);
        }

        resp.body(body::Body::from_stream(self.stream))
            .unwrap_or_else(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("build FileStream responsec error: {e}"),
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
    use http::HeaderMap;
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
                FileStream::<ReaderStream<File>>::from_path(Path::new("CHANGELOG.md"))
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

    #[tokio::test]
    async fn response_range_file() -> Result<(), Box<dyn std::error::Error>> {
        let app = Router::new().route("/range_response", get(range_stream));

        // Simulating a GET request
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/range_response")
                    .header(header::RANGE, "bytes=20-1000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Validate Response Status Code
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);

        // Validate Response Headers
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/octet-stream"
        );

        let file = File::open("CHANGELOG.md").await.unwrap();
        // get file size
        let content_length = file.metadata().await.unwrap().len();

        assert_eq!(
            response
                .headers()
                .get("content-range")
                .unwrap()
                .to_str()
                .unwrap(),
            format!("bytes 20-1000/{content_length}")
        );
        Ok(())
    }

    async fn range_stream(headers: HeaderMap) -> Response {
        let range_header = headers
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok());

        let (start, end) = if let Some(range) = range_header {
            if let Some(range) = parse_range_header(range) {
                range
            } else {
                return (StatusCode::RANGE_NOT_SATISFIABLE, "Invalid Range").into_response();
            }
        } else {
            (0, 0) // default range end = 0, if end = 0 end == file size - 1
        };

        FileStream::<ReaderStream<File>>::try_range_response(Path::new("CHANGELOG.md"), start, end)
            .await
            .unwrap()
    }

    fn parse_range_header(range: &str) -> Option<(u64, u64)> {
        let range = range.strip_prefix("bytes=")?;
        let mut parts = range.split('-');
        let start = parts.next()?.parse::<u64>().ok()?;
        let end = parts
            .next()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        if start > end {
            return None;
        }
        Some((start, end))
    }
}
