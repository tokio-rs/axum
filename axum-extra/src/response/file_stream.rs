use axum_core::{
    body,
    response::{IntoResponse, Response},
    BoxError,
};
use bytes::Bytes;
use futures_core::TryStream;
use futures_util::{stream, TryStreamExt};
use http::{header, HeaderValue, StatusCode};
use std::{
    io,
    path::{Path, PathBuf},
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt},
};
use tokio_util::io::ReaderStream;

mod range;

use self::range::{normalize_range_specs, RangeSpecs, MAX_RANGES};

/// Encapsulate the file stream.
///
/// The encapsulated file stream construct requires passing in a stream.
///
/// # Examples
///
/// ```
/// use axum::{
///     http::StatusCode,
///     response::{IntoResponse, Response},
///     routing::get,
///     Router,
/// };
/// use axum_extra::response::file_stream::FileStream;
/// use tokio::fs::File;
/// use tokio_util::io::ReaderStream;
///
/// async fn file_stream() -> Result<Response, (StatusCode, String)> {
///     let file = File::open("test.txt")
///         .await
///         .map_err(|e| (StatusCode::NOT_FOUND, format!("File not found: {e}")))?;
///
///     let stream = ReaderStream::new(file);
///     let file_stream_resp = FileStream::new(stream).file_name("test.txt");
///
///     Ok(file_stream_resp.into_response())
/// }
///
/// let app = Router::new().route("/file-stream", get(file_stream));
/// # let _: Router = app;
/// ```
#[must_use]
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
    /// Create a new [`FileStream`]
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            file_name: None,
            content_size: None,
        }
    }

    /// Set the file name of the [`FileStream`].
    ///
    /// This adds the attachment `Content-Disposition` header with the given `file_name`.
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
    ///     response::IntoResponse,
    ///     routing::get,
    ///     Router,
    /// };
    /// use axum_extra::response::file_stream::FileStream;
    /// use tokio::fs::File;
    /// use tokio::io::AsyncSeekExt;
    /// use tokio_util::io::ReaderStream;
    ///
    /// async fn range_response() -> Result<impl IntoResponse, (StatusCode, String)> {
    ///     let mut file = File::open("test.txt")
    ///         .await
    ///         .map_err(|e| (StatusCode::NOT_FOUND, format!("File not found: {e}")))?;
    ///     let mut file_size = file
    ///         .metadata()
    ///         .await
    ///         .map_err(|e| (StatusCode::NOT_FOUND, format!("Get file size: {e}")))?
    ///         .len();
    ///
    ///     file.seek(std::io::SeekFrom::Start(10))
    ///         .await
    ///         .map_err(|e| (StatusCode::NOT_FOUND, format!("File seek error: {e}")))?;
    ///     let stream = ReaderStream::new(file);
    ///
    ///     Ok(FileStream::new(stream).into_range_response(10, file_size - 1, file_size))
    /// }
    ///
    /// let app = Router::new().route("/file-stream", get(range_response));
    /// # let _: Router = app;
    /// ```
    pub fn into_range_response(self, start: u64, end: u64, total_size: u64) -> Response {
        let mut resp = Response::builder().header(header::CONTENT_TYPE, "application/octet-stream");
        resp = resp.status(StatusCode::PARTIAL_CONTENT);
        resp = resp.header(header::ACCEPT_RANGES, "bytes");
        resp = resp.header(header::CONTENT_LENGTH, end - start + 1);

        resp = resp.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total_size}"),
        );

        resp.body(body::Body::from_stream(self.stream))
            .unwrap_or_else(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("build FileStream response error: {e}"),
                )
                    .into_response()
            })
    }

    /// Attempts to return a response for an HTTP `Range` header directly from the file path.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The path of the file to be streamed
    /// * `range` - The value returned by `headers.get(header::RANGE)`
    ///
    /// # Examples
    ///
    /// ```
    /// use axum::{
    ///     http::{header, HeaderMap, StatusCode},
    ///     response::IntoResponse,
    ///     Router,
    ///     routing::get
    /// };
    /// use axum_extra::response::file_stream::FileStream;
    /// use tokio::fs::File;
    /// use tokio_util::io::ReaderStream;
    ///
    /// async fn range_stream(headers: HeaderMap) -> impl IntoResponse {
    ///     FileStream::<ReaderStream<File>>::try_range_response(
    ///         "CHANGELOG.md",
    ///         headers.get(header::RANGE),
    ///     ).await
    ///         .map_err(|e| (StatusCode::NOT_FOUND, format!("File not found: {e}")))
    /// }
    ///
    /// let app = Router::new().route("/file-stream", get(range_stream));
    /// # let _: Router = app;
    /// ```
    pub async fn try_range_response(
        file_path: impl AsRef<Path>,
        range: Option<&HeaderValue>,
    ) -> io::Result<Response> {
        let path = file_path.as_ref();
        let Some(range) = range else {
            return full_file_response(path).await;
        };

        let range_values = match range
            .to_str()
            .ok()
            .and_then(|range| range.trim().split_once('='))
        {
            Some((unit, _)) if !unit.trim().eq_ignore_ascii_case("bytes") => {
                return full_file_response(path).await;
            }
            Some((_, range_values)) => Some(range_values),
            None => None,
        };

        let file = File::open(path).await?;
        let metadata = file.metadata().await?;
        let total_size = metadata.len();
        let Some(range_values) = range_values else {
            return Ok(response_416(total_size));
        };
        let Ok(range_specs) = RangeSpecs::try_from(range_values) else {
            return Ok(response_416(total_size));
        };
        let (ranges, len) = normalize_range_specs(&range_specs, total_size);

        if len == 0 {
            return Ok(response_416(total_size));
        }

        if len == 1 {
            let (start, end) = ranges[0];
            return single_range_response(file, start, end, total_size).await;
        }

        Ok(multipart_range_response(
            path.to_owned(),
            ranges,
            len,
            total_size,
        ))
    }
}

async fn full_file_response(path: impl AsRef<Path>) -> io::Result<Response> {
    Ok(FileStream::<ReaderStream<File>>::from_path(path)
        .await?
        .into_response())
}

async fn single_range_response(
    mut file: File,
    start: u64,
    end: u64,
    total_size: u64,
) -> io::Result<Response> {
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let stream = ReaderStream::new(file.take(end - start + 1));
    Ok(FileStream::new(stream).into_range_response(start, end, total_size))
}

fn multipart_range_response(
    path: PathBuf,
    ranges: [(u64, u64); MAX_RANGES],
    len: usize,
    total_size: u64,
) -> Response {
    let boundary = multipart_boundary(total_size, len);
    let content_type = format!("multipart/byteranges; boundary={boundary}");
    let multipart_stream = stream::try_unfold(
        (
            path,
            ranges,
            len,
            total_size,
            boundary,
            0usize,
            0u8,
            None::<ReaderStream<tokio::io::Take<File>>>,
        ),
        |(path, ranges, len, total_size, boundary, index, phase, reader)| async move {
            if index == len {
                if phase == 2 {
                    return Ok::<_, io::Error>(None);
                }

                return Ok::<_, io::Error>(Some((
                    Bytes::from(format!("--{boundary}--\r\n")),
                    (path, ranges, len, total_size, boundary, index, 2, None),
                )));
            }

            let (start, end) = ranges[index];
            if phase == 0 {
                return Ok::<_, io::Error>(Some((
                    Bytes::from(format!(
                        "--{boundary}\r\nContent-Type: application/octet-stream\r\nContent-Range: bytes {start}-{end}/{total_size}\r\n\r\n"
                    )),
                    (path, ranges, len, total_size, boundary, index, 1, None),
                )));
            }

            let mut reader = if let Some(reader) = reader {
                reader
            } else {
                let mut file = File::open(&path).await?;
                file.seek(std::io::SeekFrom::Start(start)).await?;
                ReaderStream::new(file.take(end - start + 1))
            };

            match reader.try_next().await? {
                Some(bytes) => Ok::<_, io::Error>(Some((
                    bytes,
                    (
                        path,
                        ranges,
                        len,
                        total_size,
                        boundary,
                        index,
                        1,
                        Some(reader),
                    ),
                ))),
                None => Ok::<_, io::Error>(Some((
                    Bytes::from_static(b"\r\n"),
                    (path, ranges, len, total_size, boundary, index + 1, 0, None),
                ))),
            }
        },
    );

    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::ACCEPT_RANGES, "bytes")
        .body(body::Body::from_stream(multipart_stream))
        .unwrap_or_else(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("build FileStream response error: {e}"),
            )
                .into_response()
        })
}

fn multipart_boundary(total_size: u64, len: usize) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("axum-extra-boundary-{total_size}-{len}-{nanos}")
}

fn response_416(total_size: u64) -> Response {
    let mut response = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
    if let Ok(content_range) = HeaderValue::try_from(format!("bytes */{total_size}")) {
        response
            .headers_mut()
            .insert(header::CONTENT_RANGE, content_range);
    }
    response
}

// Split because the general impl requires to specify `S` and this one does not.
impl FileStream<ReaderStream<File>> {
    /// Create a [`FileStream`] from a file path.
    ///
    /// # Examples
    ///
    /// ```
    /// use axum::{
    ///     http::StatusCode,
    ///     response::IntoResponse,
    ///     Router,
    ///     routing::get
    /// };
    /// use axum_extra::response::file_stream::FileStream;
    ///
    /// async fn file_stream() -> impl IntoResponse {
    ///     FileStream::from_path("test.txt")
    ///         .await
    ///         .map_err(|e| (StatusCode::NOT_FOUND, format!("File not found: {e}")))
    /// }
    ///
    /// let app = Router::new().route("/file-stream", get(file_stream));
    /// # let _: Router = app;
    /// ```
    pub async fn from_path(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = File::open(&path).await?;
        let mut content_size = None;
        let mut file_name = None;

        if let Ok(metadata) = file.metadata().await {
            content_size = Some(metadata.len());
        }

        if let Some(file_name_os) = path.as_ref().file_name() {
            if let Some(file_name_str) = file_name_os.to_str() {
                file_name = Some(file_name_str.to_owned());
            }
        }

        Ok(Self {
            stream: ReaderStream::new(file),
            file_name,
            content_size,
        })
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
                format!(
                    "attachment; filename=\"{}\"",
                    super::content_disposition::EscapedQuotedString(&file_name)
                ),
            );
        }

        if let Some(content_size) = self.content_size {
            resp = resp.header(header::CONTENT_LENGTH, content_size);
        }

        resp.body(body::Body::from_stream(self.stream))
            .unwrap_or_else(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("build FileStream response error: {e}"),
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
    use std::io::{Cursor, Write};
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
                FileStream::from_path(Path::new("CHANGELOG.md"))
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
    async fn response_range_header_single_ranges() -> Result<(), Box<dyn std::error::Error>> {
        let file = test_file(b"0123456789")?;

        for (range_header, content_range, expected_body) in [
            ("bytes=0-0", "bytes 0-0/10", "0"),
            ("bytes=4-", "bytes 4-9/10", "456789"),
            ("bytes=-4", "bytes 6-9/10", "6789"),
            ("bytes=-999", "bytes 0-9/10", "0123456789"),
            ("bytes=0-999", "bytes 0-9/10", "0123456789"),
        ] {
            let response = response_from_range_header(file.path(), Some(range_header)).await?;
            let headers = response.headers().clone();
            assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
            assert_eq!(headers.get(header::ACCEPT_RANGES).unwrap(), "bytes");
            assert_eq!(headers.get(header::CONTENT_RANGE).unwrap(), content_range);
            assert_eq!(
                headers.get(header::CONTENT_LENGTH).unwrap(),
                &expected_body.len().to_string()
            );
            assert_eq!(body_text(response).await?, expected_body);
        }

        Ok(())
    }

    #[tokio::test]
    async fn response_range_header_unsatisfiable() -> Result<(), Box<dyn std::error::Error>> {
        let file = test_file(b"0123456789")?;
        let response = response_from_range_header(file.path(), Some("bytes=99-100")).await?;

        assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(
            response.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes */10"
        );
        assert_eq!(body_text(response).await?, "");
        Ok(())
    }

    #[tokio::test]
    async fn response_range_header_malformed() -> Result<(), Box<dyn std::error::Error>> {
        let file = test_file(b"0123456789")?;

        for range_header in [
            "bytes=5-3",
            "bytes=-0",
            "bytes=abc-def",
            "bytes=9223372036854775808-",
        ] {
            let response = response_from_range_header(file.path(), Some(range_header)).await?;
            assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
            assert_eq!(
                response.headers().get(header::CONTENT_RANGE).unwrap(),
                "bytes */10"
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn response_range_header_limits_ranges() -> Result<(), Box<dyn std::error::Error>> {
        let file = test_file(b"0123456789")?;
        let response = response_from_range_header(
            file.path(),
            Some("bytes=0-0,1-1,2-2,3-3,4-4,5-5,6-6,7-7,8-8"),
        )
        .await?;

        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        let body = body_text(response).await?;
        assert_eq!(body.matches("Content-Range:").count(), MAX_RANGES);
        assert!(body.contains("Content-Range: bytes 7-7/10"));
        assert!(!body.contains("Content-Range: bytes 8-8/10"));
        Ok(())
    }

    #[tokio::test]
    async fn response_range_header_unknown_unit() -> Result<(), Box<dyn std::error::Error>> {
        let file = test_file(b"0123456789")?;

        for range_header in [None, Some("items=0-3")] {
            let response = response_from_range_header(file.path(), range_header).await?;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(body_text(response).await?, "0123456789");
        }

        Ok(())
    }

    #[tokio::test]
    async fn response_range_header_multiple_ranges() -> Result<(), Box<dyn std::error::Error>> {
        let file = test_file(b"0123456789")?;
        let response = response_from_range_header(file.path(), Some("bytes=0-0,8-9")).await?;
        let headers = response.headers().clone();
        let content_type = headers
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()?
            .to_owned();
        let boundary = content_type
            .strip_prefix("multipart/byteranges; boundary=")
            .unwrap();

        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(headers.get(header::ACCEPT_RANGES).unwrap(), "bytes");
        assert!(headers.get(header::CONTENT_RANGE).is_none());

        let body = body_text(response).await?;
        assert!(body.contains(&format!(
            "--{boundary}\r\nContent-Type: application/octet-stream\r\nContent-Range: bytes 0-0/10\r\n\r\n0\r\n"
        )));
        assert!(body.contains(&format!(
            "--{boundary}\r\nContent-Type: application/octet-stream\r\nContent-Range: bytes 8-9/10\r\n\r\n89\r\n"
        )));
        assert!(body.ends_with(&format!("--{boundary}--\r\n")));
        Ok(())
    }

    #[tokio::test]
    async fn filename_escapes_quotes() -> Result<(), Box<dyn std::error::Error>> {
        let app = Router::new().route(
            "/file",
            get(|| async {
                let file_content = b"data".to_vec();
                let reader = Cursor::new(file_content);
                let stream = ReaderStream::new(reader);
                // Filename containing double quotes that could cause parameter injection
                FileStream::new(stream)
                    .file_name("evil\"; filename*=UTF-8''pwned.txt; x=\"")
                    .into_response()
            }),
        );

        let response = app
            .oneshot(Request::builder().uri("/file").body(Body::empty())?)
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-disposition").unwrap(),
            "attachment; filename=\"evil\\\"; filename*=UTF-8''pwned.txt; x=\\\"\""
        );
        Ok(())
    }

    #[tokio::test]
    async fn filename_escapes_backslashes() -> Result<(), Box<dyn std::error::Error>> {
        let app = Router::new().route(
            "/file",
            get(|| async {
                let file_content = b"data".to_vec();
                let reader = Cursor::new(file_content);
                let stream = ReaderStream::new(reader);
                FileStream::new(stream)
                    .file_name("file\\name.txt")
                    .into_response()
            }),
        );

        let response = app
            .oneshot(Request::builder().uri("/file").body(Body::empty())?)
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-disposition").unwrap(),
            "attachment; filename=\"file\\\\name.txt\""
        );
        Ok(())
    }

    #[tokio::test]
    async fn response_range_empty_file() -> Result<(), Box<dyn std::error::Error>> {
        let file = tempfile::NamedTempFile::new()?;
        file.as_file().set_len(0)?;
        let path = file.path().to_owned();
        let range = HeaderValue::from_static("bytes=0-");

        let response = FileStream::<ReaderStream<File>>::try_range_response(path, Some(&range))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(
            response.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes */0"
        );
        Ok(())
    }

    fn test_file(contents: &[u8]) -> Result<tempfile::NamedTempFile, Box<dyn std::error::Error>> {
        let mut file = tempfile::NamedTempFile::new()?;
        file.write_all(contents)?;
        Ok(file)
    }

    async fn response_from_range_header(
        path: &Path,
        range: Option<&'static str>,
    ) -> Result<Response, Box<dyn std::error::Error>> {
        let range = range.map(HeaderValue::from_static);
        Ok(FileStream::<ReaderStream<File>>::try_range_response(path, range.as_ref()).await?)
    }

    async fn body_text(response: Response) -> Result<String, Box<dyn std::error::Error>> {
        Ok(String::from_utf8(
            response.into_body().collect().await?.to_bytes().to_vec(),
        )?)
    }
}
