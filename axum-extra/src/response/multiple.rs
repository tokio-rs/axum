//! Generate forms to use in responses.

use axum::response::{IntoResponse, Response};
use fastrand;
use http::{header, HeaderMap, StatusCode};
use mime::Mime;

/// Create multipart forms to be used in API responses.
///
/// This struct implements [`IntoResponse`], and so it can be returned from a handler.
#[derive(Debug)]
pub struct MultipartForm {
    parts: Vec<Part>,
}

impl MultipartForm {
    /// Initialize a new multipart form with the provided vector of parts.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use axum_extra::response::multiple::{MultipartForm, Part};
    ///
    /// let parts: Vec<Part> = vec![Part::text("foo".to_string(), "abc"), Part::text("bar".to_string(), "def")];
    /// let form = MultipartForm::with_parts(parts);
    /// ```
    pub fn with_parts(parts: Vec<Part>) -> Self {
        MultipartForm { parts }
    }
}

impl IntoResponse for MultipartForm {
    fn into_response(self) -> Response {
        // see RFC5758 for details
        let boundary = generate_boundary();
        let mut headers = HeaderMap::new();
        let mime_type: Mime = match format!("multipart/form-data; boundary={boundary}").parse() {
            Ok(m) => m,
            // Realistically this should never happen unless the boundary generation code
            // is modified, and that will be caught by unit tests
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Invalid multipart boundary generated",
                )
                    .into_response()
            }
        };
        // The use of unwrap is safe here because mime types are inherently string representable
        headers.insert(header::CONTENT_TYPE, mime_type.to_string().parse().unwrap());
        let mut serialized_form: Vec<u8> = Vec::new();
        for part in self.parts {
            // for each part, the boundary is preceded by two dashes
            serialized_form.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            serialized_form.extend_from_slice(&part.serialize());
        }
        serialized_form.extend_from_slice(format!("--{boundary}--").as_bytes());
        (headers, serialized_form).into_response()
    }
}

// Valid settings for that header are: "base64", "quoted-printable", "8bit", "7bit", and "binary".
/// A single part of a multipart form as defined by
/// <https://www.w3.org/TR/html401/interact/forms.html#h-17.13.4>
/// and RFC5758.
#[derive(Debug)]
pub struct Part {
    // Every part is expected to contain:
    // - a [Content-Disposition](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Disposition
    // header, where `Content-Disposition` is set to `form-data`, with a parameter of `name` that is set to
    // the name of the field in the form. In the below example, the name of the field is `user`:
    // ```
    // Content-Disposition: form-data; name="user"
    // ```
    // If the field contains a file, then the `filename` parameter may be set to the name of the file.
    // Handling for non-ascii field names is not done here, support for non-ascii characters may be encoded using
    // methodology described in RFC 2047.
    // - (optionally) a `Content-Type` header, which if not set, defaults to `text/plain`.
    // If the field contains a file, then the file should be identified with that file's MIME type (eg: `image/gif`).
    // If the `MIME` type is not known or specified, then the MIME type should be set to `application/octet-stream`.
    /// The name of the part in question
    name: String,
    /// If the part should be treated as a file, the filename that should be attached that part
    filename: Option<String>,
    /// The `Content-Type` header. While not strictly required, it is always set here
    mime_type: Mime,
    /// The content/body of the part
    contents: Vec<u8>,
}

impl Part {
    /// Create a new part with `Content-Type` of `text/plain` with the supplied name and contents.
    ///
    /// This form will not have a defined file name.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use axum_extra::response::multiple::{MultipartForm, Part};
    ///
    /// // create a form with a single part that has a field with a name of "foo",
    /// // and a value of "abc"
    /// let parts: Vec<Part> = vec![Part::text("foo".to_string(), "abc")];
    /// let form = MultipartForm::from_iter(parts);
    /// ```
    pub fn text(name: String, contents: &str) -> Self {
        Self {
            name,
            filename: None,
            mime_type: mime::TEXT_PLAIN_UTF_8,
            contents: contents.as_bytes().to_vec(),
        }
    }

    /// Create a new part containing a generic file, with a `Content-Type` of `application/octet-stream`
    /// using the provided file name, field name, and contents.
    ///
    /// If the MIME type of the file is known, consider using `Part::raw_part`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use axum_extra::response::multiple::{MultipartForm, Part};
    ///
    /// // create a form with a single part that has a field with a name of "foo",
    /// // with a file name of "foo.txt", and with the specified contents
    /// let parts: Vec<Part> = vec![Part::file("foo", "foo.txt", vec![0x68, 0x68, 0x20, 0x6d, 0x6f, 0x6d])];
    /// let form = MultipartForm::from_iter(parts);
    /// ```
    pub fn file(field_name: &str, file_name: &str, contents: Vec<u8>) -> Self {
        Self {
            name: field_name.to_owned(),
            filename: Some(file_name.to_owned()),
            // If the `MIME` type is not known or specified, then the MIME type should be set to `application/octet-stream`.
            // See RFC2388 section 3 for specifics.
            mime_type: mime::APPLICATION_OCTET_STREAM,
            contents,
        }
    }

    /// Create a new part with more fine-grained control over the semantics of that part.
    ///
    /// The caller is assumed to have set a valid MIME type.
    ///
    /// This function will return an error if the provided MIME type is not valid.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use axum_extra::response::multiple::{MultipartForm, Part};
    ///
    /// // create a form with a single part that has a field with a name of "part_name",
    /// // with a MIME type of "application/json", and the supplied contents.
    /// let parts: Vec<Part> = vec![Part::raw_part("part_name", "application/json", vec![0x68, 0x68, 0x20, 0x6d, 0x6f, 0x6d], None).expect("MIME type must be valid")];
    /// let form = MultipartForm::from_iter(parts);
    /// ```
    pub fn raw_part(
        name: &str,
        mime_type: &str,
        contents: Vec<u8>,
        filename: Option<&str>,
    ) -> Result<Self, &'static str> {
        let mime_type = mime_type.parse().map_err(|_| "Invalid MIME type")?;
        Ok(Self {
            name: name.to_owned(),
            filename: filename.map(|f| f.to_owned()),
            mime_type,
            contents,
        })
    }

    /// Serialize this part into a chunk that can be easily inserted into a larger form
    pub(super) fn serialize(&self) -> Vec<u8> {
        // A part is serialized in this general format:
        // // the filename is optional
        // Content-Disposition: form-data; name="FIELD_NAME"; filename="FILENAME"\r\n
        // // the mime type (not strictly required by the spec, but always sent here)
        // Content-Type: mime/type\r\n
        // // a blank line, then the contents of the file start
        // \r\n
        // CONTENTS\r\n

        // Format what we can as a string, then handle the rest at a byte level
        let mut serialized_part = format!("Content-Disposition: form-data; name=\"{}\"", self.name);
        // specify a filename if one was set
        if let Some(filename) = &self.filename {
            serialized_part += &format!("; filename=\"{filename}\"");
        }
        serialized_part += "\r\n";
        // specify the MIME type
        serialized_part += &format!("Content-Type: {}\r\n", self.mime_type);
        serialized_part += "\r\n";
        let mut part_bytes = serialized_part.as_bytes().to_vec();
        part_bytes.extend_from_slice(&self.contents);
        part_bytes.extend_from_slice(b"\r\n");

        part_bytes
    }
}

impl FromIterator<Part> for MultipartForm {
    fn from_iter<T: IntoIterator<Item = Part>>(iter: T) -> Self {
        Self {
            parts: iter.into_iter().collect(),
        }
    }
}

/// A boundary is defined as a user defined (arbitrary) value that does not occur in any of the data.
///
/// Because the specification does not clearly define a methodology for generating boundaries, this implementation
/// follow's Reqwest's, and generates a boundary in the format of `XXXXXXXX-XXXXXXXX-XXXXXXXX-XXXXXXXX` where `XXXXXXXX`
/// is a hexadecimal representation of a pseudo randomly generated u64.
fn generate_boundary() -> String {
    let a = fastrand::u64(0..u64::MAX);
    let b = fastrand::u64(0..u64::MAX);
    let c = fastrand::u64(0..u64::MAX);
    let d = fastrand::u64(0..u64::MAX);
    format!("{a:016x}-{b:016x}-{c:016x}-{d:016x}")
}

#[cfg(test)]
mod tests {
    use super::{generate_boundary, MultipartForm, Part};
    use axum::{body::Body, http};
    use axum::{routing::get, Router};
    use http::{Request, Response};
    use http_body_util::BodyExt;
    use mime::Mime;
    use tower::ServiceExt;

    #[tokio::test]
    async fn process_form() -> Result<(), Box<dyn std::error::Error>> {
        // create a boilerplate handle that returns a form
        async fn handle() -> MultipartForm {
            let parts: Vec<Part> = vec![
                Part::text("part1".to_owned(), "basictext"),
                Part::file(
                    "part2",
                    "file.txt",
                    vec![0x68, 0x69, 0x20, 0x6d, 0x6f, 0x6d],
                ),
                Part::raw_part("part3", "text/plain", b"rawpart".to_vec(), None).unwrap(),
            ];
            MultipartForm::from_iter(parts)
        }

        // make a request to that handle
        let app = Router::new().route("/", get(handle));
        let response: Response<_> = app
            .oneshot(Request::builder().uri("/").body(Body::empty())?)
            .await?;
        // content_type header
        let ct_header = response.headers().get("content-type").unwrap().to_str()?;
        let boundary = ct_header.split("boundary=").nth(1).unwrap().to_owned();
        let body: &[u8] = &response.into_body().collect().await?.to_bytes();
        assert_eq!(
            std::str::from_utf8(body)?,
            format!(
                "--{boundary}\r\n\
                Content-Disposition: form-data; name=\"part1\"\r\n\
                Content-Type: text/plain; charset=utf-8\r\n\
                \r\n\
                basictext\r\n\
                --{boundary}\r\n\
                Content-Disposition: form-data; name=\"part2\"; filename=\"file.txt\"\r\n\
                Content-Type: application/octet-stream\r\n\
                \r\n\
                hi mom\r\n\
                --{boundary}\r\n\
                Content-Disposition: form-data; name=\"part3\"\r\n\
                Content-Type: text/plain\r\n\
                \r\n\
                rawpart\r\n\
                --{boundary}--",
            )
        );

        Ok(())
    }

    #[test]
    fn valid_boundary_generation() {
        for _ in 0..256 {
            let boundary = generate_boundary();
            let mime_type: Result<Mime, _> =
                format!("multipart/form-data; boundary={boundary}").parse();
            assert!(
                mime_type.is_ok(),
                "The generated boundary was unable to be parsed into a valid mime type."
            );
        }
    }
}
