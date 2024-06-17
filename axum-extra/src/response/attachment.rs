use axum::response::IntoResponse;
use http::{header, HeaderMap, HeaderValue};
use tracing::trace;

/// A file attachment response type
///
/// This type will set the `Content-Disposition` header for this response. In response a webbrowser
/// will download the contents localy.
///
/// Use the `filename` and `content_type` methods to set the filename or content-type of the
/// attachment. If these values are not set they will not be sent.
///
///
/// # Example
///
/// ```rust
///  use axum::{http::StatusCode, routing::get, Router};
///  use axum_extra::response::Attachment;
///
///  async fn cargo_toml() -> Result<Attachment<String>, (StatusCode, String)> {
///      let file_contents = tokio::fs::read_to_string("Cargo.toml")
///          .await
///          .map_err(|err| (StatusCode::NOT_FOUND, format!("File not found: {err}")))?;
///      Ok(Attachment::new(file_contents)
///          .filename("Cargo.toml")
///          .content_type("text/x-toml"))
///  }
///
///  let app = Router::new().route("/Cargo.toml", get(cargo_toml));
///  let _: Router = app;
/// ```
///
/// Hyper will set the `Content-Length` header if it knows the length. To manually set this header
/// use the [`Attachment`] type in a tuple.
///
/// # Note
/// If the content length is known and this header is manualy set to a different length hyper
/// panics.
///
/// ```rust
/// async fn with_content_length() -> impl IntoResponse {
///     (
///         [(header::CONTENT_LENGTH, 3)],
///         Attachment::new([0, 0, 0])
///             .filename("Cargo.toml")
///             .content_type("text/x-toml"),
///     )
/// }
/// ```
#[derive(Debug)]
pub struct Attachment<T> {
    inner: T,
    filename: Option<HeaderValue>,
    content_type: Option<HeaderValue>,
}

impl<T: IntoResponse> Attachment<T> {
    /// Creates a new [`Attachment`].
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            filename: None,
            content_type: None,
        }
    }

    /// Sets the filename of the [`Attachment`]
    ///
    /// This updates the `Content-Disposition` header to add a filename.
    pub fn filename<H: TryInto<HeaderValue>>(mut self, value: H) -> Self {
        if let Some(filename) = value.try_into().ok() {
            self.filename = Some(filename);
        } else {
            trace!("Attachment filename contains invalid characters");
        }
        self
    }

    /// Sets the content-type of the [`Attachment`]
    pub fn content_type<H: TryInto<HeaderValue>>(mut self, value: H) -> Self {
        if let Some(content_type) = value.try_into().ok() {
            self.content_type = Some(content_type);
        } else {
            trace!("Attachment content-type contains invalid characters");
        }
        self
    }
}

impl<T> IntoResponse for Attachment<T>
where
    T: IntoResponse,
{
    fn into_response(self) -> axum::response::Response {
        let mut headers = HeaderMap::new();

        if let Some(content_type) = self.content_type {
            headers.append(header::CONTENT_TYPE, content_type);
        }

        if let Some(filename) = self.filename {
            let mut bytes = b"attachment; filename=\"".to_vec();
            bytes.extend_from_slice(filename.as_bytes());
            bytes.push(b'\"');

            let content_disposition = HeaderValue::from_bytes(&bytes)
                .expect("This was a HeaderValue so this can not fail");

            headers.append(header::CONTENT_DISPOSITION, content_disposition);
        } else {
            headers.append(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment"),
            );
        }

        (headers, self.inner).into_response()
    }
}
