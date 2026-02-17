use super::content_disposition::EscapedFilename;
use axum_core::response::IntoResponse;
use http::{header, HeaderMap, HeaderValue};
use tracing::error;

/// A file attachment response.
///
/// This type will set the `Content-Disposition` header to `attachment`. In response a webbrowser
/// will offer to download the file instead of displaying it directly.
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
/// # Note
///
/// If you use axum with hyper, hyper will set the `Content-Length` if it is known.
#[derive(Debug)]
#[must_use]
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

    /// Sets the filename of the [`Attachment`].
    ///
    /// This updates the `Content-Disposition` header to add a filename.
    pub fn filename<H: TryInto<HeaderValue>>(mut self, value: H) -> Self {
        self.filename = if let Ok(filename) = value.try_into() {
            Some(filename)
        } else {
            error!("Attachment filename contains invalid characters");
            None
        };
        self
    }

    /// Sets the content-type of the [`Attachment`]
    pub fn content_type<H: TryInto<HeaderValue>>(mut self, value: H) -> Self {
        if let Ok(content_type) = value.try_into() {
            self.content_type = Some(content_type);
        } else {
            error!("Attachment content-type contains invalid characters");
        }
        self
    }
}

impl<T> IntoResponse for Attachment<T>
where
    T: IntoResponse,
{
    fn into_response(self) -> axum_core::response::Response {
        let mut headers = HeaderMap::new();

        if let Some(content_type) = self.content_type {
            headers.append(header::CONTENT_TYPE, content_type);
        }

        let content_disposition = if let Some(filename) = self.filename {
            let filename_str = filename
                .to_str()
                .expect("This was a HeaderValue so this can not fail");
            let value = format!("attachment; filename=\"{}\"", EscapedFilename(filename_str));
            HeaderValue::try_from(value).expect("This was a HeaderValue so this can not fail")
        } else {
            HeaderValue::from_static("attachment")
        };

        headers.append(header::CONTENT_DISPOSITION, content_disposition);

        (headers, self.inner).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_core::response::IntoResponse;
    use http::header::CONTENT_DISPOSITION;

    #[test]
    fn attachment_without_filename() {
        let attachment = Attachment::new("data").into_response();
        let value = attachment.headers().get(CONTENT_DISPOSITION).unwrap();
        assert_eq!(value, "attachment");
    }

    #[test]
    fn attachment_with_normal_filename() {
        let attachment = Attachment::new("data")
            .filename("report.pdf")
            .into_response();
        let value = attachment.headers().get(CONTENT_DISPOSITION).unwrap();
        assert_eq!(value, "attachment; filename=\"report.pdf\"");
    }

    #[test]
    fn attachment_filename_escapes_quotes() {
        // A filename containing a double quote should be escaped to prevent
        // Content-Disposition parameter injection (see CVE-2023-29401)
        let attachment = Attachment::new("data")
            .filename("evil\"; filename*=UTF-8''pwned.txt; x=\"")
            .into_response();
        let value = attachment.headers().get(CONTENT_DISPOSITION).unwrap();
        assert_eq!(
            value,
            "attachment; filename=\"evil\\\"; filename*=UTF-8''pwned.txt; x=\\\"\""
        );
    }

    #[test]
    fn attachment_filename_escapes_backslashes() {
        let attachment = Attachment::new("data")
            .filename("file\\name.txt")
            .into_response();
        let value = attachment.headers().get(CONTENT_DISPOSITION).unwrap();
        assert_eq!(value, "attachment; filename=\"file\\\\name.txt\"");
    }
}
