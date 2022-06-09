use crate::body::AsyncReadBody;
use axum::response::{IntoResponse, Response};
use http::header::{HeaderValue, CONTENT_TYPE, LAST_MODIFIED};
use httpdate::HttpDate;
use std::{io, path::Path};
use tokio::fs::File;

/// A response created from a file.
///
/// Note that if you need more complex features such as support for range requests, precompressed
/// files, and `If-Not-Modified` consider using [`tower_http::services::ServeFile`] instead.
///
/// [`tower_http::services::ServeFile`]: https://docs.rs/tower-http/latest/tower_http/services/struct.ServeFile.html
#[derive(Debug)]
pub struct FileResponse {
    file: File,
    content_type: Option<HeaderValue>,
    last_modified: Option<HttpDate>,
}

impl FileResponse {
    /// Create a new `FileResponse` by opening the file at the given path.
    ///
    /// The `Content-Type` will be inferred from the file extension.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum_extra::response::FileResponse;
    /// use axum::http::StatusCode;
    ///
    /// async fn cargo_toml() -> Result<FileResponse, StatusCode> {
    ///     FileResponse::open("Cargo.toml").await.map_err(|_| StatusCode::NOT_FOUND)
    /// }
    /// ```
    pub async fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let file = File::open(path).await?;
        Ok(Self::from_path_and_file(path, file).await)
    }

    /// Create a new `FileResponse` from a file and a path.
    ///
    /// Use this if you already have an open file and the path to it.
    ///
    /// The `Content-Type` will be inferred from `path`'s file extension.
    pub async fn from_path_and_file(path: impl AsRef<Path>, file: File) -> Self {
        let content_type = mime_guess::from_path(path)
            .first_raw()
            .map(HeaderValue::from_static)
            .unwrap_or_else(|| {
                HeaderValue::from_str(mime::APPLICATION_OCTET_STREAM.as_ref()).unwrap()
            });

        let last_modified = file
            .metadata()
            .await
            .ok()
            .and_then(|meta| meta.modified().ok())
            .map(HttpDate::from);

        Self {
            file,
            content_type: Some(content_type),
            last_modified,
        }
    }

    /// Create a new `FileResponse` from a file.
    ///
    /// Use this if you have a file but you don't its path.
    ///
    /// The response will not contain a `Content-Type` header.
    ///
    /// The response will be similar to using [`AsyncReadBody`] directly.
    pub fn from_file(file: File) -> Self {
        Self {
            file,
            content_type: None,
            last_modified: None,
        }
    }
}

impl IntoResponse for FileResponse {
    fn into_response(self) -> Response {
        let content_type = self
            .content_type
            .map(|content_type| [(CONTENT_TYPE, content_type)]);

        let last_modified = self
            .last_modified
            .map(|last_modified| [(LAST_MODIFIED, last_modified.to_string())]);

        let body = AsyncReadBody::new(self.file);

        (content_type, last_modified, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{routing::get, Router};
    use http::StatusCode;

    #[tokio::test]
    async fn works() {
        let app = Router::new().route(
            "/",
            get(|| async {
                let path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
                FileResponse::open(path)
                    .await
                    .map_err(|_| StatusCode::NOT_FOUND)
            }),
        );

        let client = TestClient::new(app);

        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()["content-type"], "text/x-toml");
        assert!(res.headers().get("last-modified").is_some());
    }
}
