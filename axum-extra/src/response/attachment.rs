use super::content_disposition::EscapedQuotedString;
use axum_core::response::IntoResponse;
use http::{header, HeaderMap, HeaderValue};
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use tracing::error;

// Encode everything except RFC 8187's attr-char characters.
const FILENAME_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'!')
    .remove(b'#')
    .remove(b'$')
    .remove(b'&')
    .remove(b'+')
    .remove(b'-')
    .remove(b'.')
    .remove(b'^')
    .remove(b'_')
    .remove(b'`')
    .remove(b'|')
    .remove(b'~');

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
    ///
    /// Non-ASCII filenames use a UTF-8 encoded `filename*` parameter, with an ASCII
    /// `filename` fallback for clients that do not support it. In the fallback,
    /// each non-ASCII character and each `%` is replaced with `?`.
    /// ASCII filenames use only the `filename` parameter.
    pub fn filename<H: TryInto<HeaderValue>>(mut self, value: H) -> Self {
        self.filename = match value.try_into() {
            Ok(filename) if std::str::from_utf8(filename.as_bytes()).is_ok() => Some(filename),
            _ => {
                error!("Attachment filename contains invalid characters");
                None
            }
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
            let filename_str = std::str::from_utf8(filename.as_bytes())
                .expect("filename was checked to be valid UTF-8");
            let value = if filename_str.is_ascii() {
                format!(
                    "attachment; filename=\"{}\"",
                    EscapedQuotedString(filename_str)
                )
            } else {
                let fallback: String = filename_str
                    .chars()
                    .map(|c| if c.is_ascii() && c != '%' { c } else { '?' })
                    .collect();
                format!(
                    "attachment; filename=\"{}\"; filename*=UTF-8''{}",
                    EscapedQuotedString(&fallback),
                    utf8_percent_encode(filename_str, FILENAME_ENCODE_SET)
                )
            };
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
    fn attachment_with_non_ascii_filename() {
        let attachment = Attachment::new("data")
            .filename("Bericht-März.pdf")
            .into_response();
        let value = attachment.headers().get(CONTENT_DISPOSITION).unwrap();
        assert_eq!(
            std::str::from_utf8(value.as_bytes()).unwrap(),
            "attachment; filename=\"Bericht-M?rz.pdf\"; filename*=UTF-8''Bericht-M%C3%A4rz.pdf"
        );
    }

    #[test]
    fn attachment_with_opaque_filename() {
        let filename = HeaderValue::from_bytes(b"report\xff.pdf").unwrap();
        let attachment = Attachment::new("data").filename(filename).into_response();
        let value = attachment.headers().get(CONTENT_DISPOSITION).unwrap();
        assert_eq!(value, "attachment");
    }

    #[test]
    fn attachment_with_extended_filename() {
        for (filename, expected) in [
            (
                "résumé 100%.txt",
                "attachment; filename=\"r?sum? 100?.txt\"; filename*=UTF-8''r%C3%A9sum%C3%A9%20100%25.txt",
            ),
            (
                "🦀.txt",
                "attachment; filename=\"?.txt\"; filename*=UTF-8''%F0%9F%A6%80.txt",
            ),
            (
                "éAZaz09!#$&+-.^_`|~.txt",
                "attachment; filename=\"?AZaz09!#$&+-.^_`|~.txt\"; filename*=UTF-8''%C3%A9AZaz09!#$&+-.^_`|~.txt",
            ),
            (
                "é \"\\%*';/?:@=[]{}()<>,.txt",
                concat!(
                    "attachment; filename=\"? \\\"\\\\?*';/?:@=[]{}()<>,.txt\"; ",
                    "filename*=UTF-8''%C3%A9%20%22%5C%25%2A%27%3B%2F%3F%3A%40%3D%5B%5D%7B%7D%28%29%3C%3E%2C.txt",
                ),
            ),
            (
                "é\t.txt",
                "attachment; filename=\"?\t.txt\"; filename*=UTF-8''%C3%A9%09.txt",
            ),
        ] {
            let response = Attachment::new("data").filename(filename).into_response();
            assert_eq!(response.headers()[CONTENT_DISPOSITION], expected, "{filename:?}");
        }
    }

    #[test]
    fn attachment_with_ascii_filename_preserves_spaces_and_percent() {
        let response = Attachment::new("data")
            .filename("report 100%20.txt")
            .into_response();
        assert_eq!(
            response.headers()[CONTENT_DISPOSITION],
            "attachment; filename=\"report 100%20.txt\""
        );
    }

    #[test]
    fn attachment_with_invalid_filename() {
        for filename in [
            "é\r\nx-injected: yes",
            "é\n.txt",
            "é\0.txt",
            "é\x1f.txt",
            "é\x7f.txt",
        ] {
            let response = Attachment::new("data").filename(filename).into_response();
            assert_eq!(response.headers()[CONTENT_DISPOSITION], "attachment");
        }
    }

    #[test]
    fn attachment_extended_filename_escapes_parameter_injection() {
        let response = Attachment::new("data")
            .filename("é\"; filename*=UTF-8''pwned.txt; x=\"")
            .into_response();
        assert_eq!(
            response.headers()[CONTENT_DISPOSITION],
            concat!(
                "attachment; filename=\"?\\\"; filename*=UTF-8''pwned.txt; x=\\\"\"; ",
                "filename*=UTF-8''%C3%A9%22%3B%20filename%2A%3DUTF-8%27%27pwned.txt%3B%20x%3D%22",
            )
        );
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
