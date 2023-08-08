//! Extractor that parses `multipart/form-data` requests commonly used with file uploads.
//!
//! See [`Multipart`] for more details.

use axum::{
    async_trait,
    body::{Bytes, HttpBody},
    extract::{BodyStream, FromRequest},
    response::{IntoResponse, Response},
    BoxError, RequestExt,
};
use futures_util::stream::Stream;
use http::{
    header::{HeaderMap, CONTENT_TYPE},
    Request, StatusCode,
};
use std::{
    error::Error,
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

/// Extractor that parses `multipart/form-data` requests (commonly used with file uploads).
///
/// ⚠️ Since extracting multipart form data from the request requires consuming the body, the
/// `Multipart` extractor must be *last* if there are multiple extractors in a handler.
/// See ["the order of extractors"][order-of-extractors]
///
/// [order-of-extractors]: crate::extract#the-order-of-extractors
///
/// # Example
///
/// ```
/// use axum::{
///     routing::post,
///     Router,
/// };
/// use axum_extra::extract::Multipart;
///
/// async fn upload(mut multipart: Multipart) {
///     while let Some(mut field) = multipart.next_field().await.unwrap() {
///         let name = field.name().unwrap().to_string();
///         let data = field.bytes().await.unwrap();
///
///         println!("Length of `{}` is {} bytes", name, data.len());
///     }
/// }
///
/// let app = Router::new().route("/upload", post(upload));
/// # let _: Router = app;
/// ```
///
/// # Field Exclusivity
///
/// A [`Field`] represents a raw, self-decoding stream into multipart data. As such, only one
/// [`Field`] from a given Multipart instance may be live at once. That is, a [`Field`] emitted by
/// [`next_field()`] must be dropped before calling [`next_field()`] again. Failure to do so will
/// result in an error.
///
/// ```
/// use axum_extra::extract::Multipart;
///
/// async fn handler(mut multipart: Multipart) {
///     let field_1 = multipart.next_field().await;
///
///     // We cannot get the next field while `field_1` is still alive. Have to drop `field_1`
///     // first.
///     let field_2 = multipart.next_field().await;
///     assert!(field_2.is_err());
/// }
/// ```
///
/// In general you should consume `Multipart` by looping over the fields in order and make sure not
/// to keep `Field`s around from previous loop iterations. That will minimize the risk of runtime
/// errors.
///
/// # Differences between this and  `axum::extract::Multipart`
///
/// `axum::extract::Multipart` uses lifetimes to enforce field exclusivity at compile time, however
/// that leads to significant usability issues such as `Field` not being `'static`.
///
/// `axum_extra::extract::Multipart` instead enforces field exclusivity at runtime which makes
/// things easier to use at the cost of possible runtime errors.
///
/// [`next_field()`]: Multipart::next_field
#[cfg_attr(docsrs, doc(cfg(feature = "multipart")))]
#[derive(Debug)]
pub struct Multipart {
    inner: multer::Multipart<'static>,
}

#[async_trait]
impl<S, B> FromRequest<S, B> for Multipart
where
    B: HttpBody + Send + 'static,
    B::Data: Into<Bytes>,
    B::Error: Into<BoxError>,
    S: Send + Sync,
{
    type Rejection = MultipartRejection;

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        let boundary = parse_boundary(req.headers()).ok_or(InvalidBoundary)?;
        let stream_result = match req.with_limited_body() {
            Ok(limited) => BodyStream::from_request(limited, state).await,
            Err(unlimited) => BodyStream::from_request(unlimited, state).await,
        };
        let stream = stream_result.unwrap_or_else(|err| match err {});
        let multipart = multer::Multipart::new(stream, boundary);
        Ok(Self { inner: multipart })
    }
}

impl Multipart {
    /// Yields the next [`Field`] if available.
    pub async fn next_field(&mut self) -> Result<Option<Field>, MultipartError> {
        let field = self
            .inner
            .next_field()
            .await
            .map_err(MultipartError::from_multer)?;

        if let Some(field) = field {
            Ok(Some(Field { inner: field }))
        } else {
            Ok(None)
        }
    }

    /// Convert the `Multipart` into a stream of its fields.
    pub fn into_stream(self) -> impl Stream<Item = Result<Field, MultipartError>> + Send + 'static {
        futures_util::stream::try_unfold(self, |mut multipart| async move {
            let field = multipart.next_field().await?;
            Ok(field.map(|field| (field, multipart)))
        })
    }
}

/// A single field in a multipart stream.
#[derive(Debug)]
pub struct Field {
    inner: multer::Field<'static>,
}

impl Stream for Field {
    type Item = Result<Bytes, MultipartError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner)
            .poll_next(cx)
            .map_err(MultipartError::from_multer)
    }
}

impl Field {
    /// The field name found in the
    /// [`Content-Disposition`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Disposition)
    /// header.
    pub fn name(&self) -> Option<&str> {
        self.inner.name()
    }

    /// The file name found in the
    /// [`Content-Disposition`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Disposition)
    /// header.
    pub fn file_name(&self) -> Option<&str> {
        self.inner.file_name()
    }

    /// Get the [content type](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Type) of the field.
    pub fn content_type(&self) -> Option<&str> {
        self.inner.content_type().map(|m| m.as_ref())
    }

    /// Get a map of headers as [`HeaderMap`].
    pub fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    /// Get the full data of the field as [`Bytes`].
    pub async fn bytes(self) -> Result<Bytes, MultipartError> {
        self.inner
            .bytes()
            .await
            .map_err(MultipartError::from_multer)
    }

    /// Get the full field data as text.
    pub async fn text(self) -> Result<String, MultipartError> {
        self.inner.text().await.map_err(MultipartError::from_multer)
    }

    /// Stream a chunk of the field data.
    ///
    /// When the field data has been exhausted, this will return [`None`].
    ///
    /// Note this does the same thing as `Field`'s [`Stream`] implementation.
    ///
    /// # Example
    ///
    /// ```
    /// use axum::{
    ///    routing::post,
    ///    response::IntoResponse,
    ///    http::StatusCode,
    ///    Router,
    /// };
    /// use axum_extra::extract::Multipart;
    ///
    /// async fn upload(mut multipart: Multipart) -> Result<(), (StatusCode, String)> {
    ///     while let Some(mut field) = multipart
    ///         .next_field()
    ///         .await
    ///         .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?
    ///     {
    ///         while let Some(chunk) = field
    ///             .chunk()
    ///             .await
    ///             .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?
    ///         {
    ///             println!("received {} bytes", chunk.len());
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    ///
    /// let app = Router::new().route("/upload", post(upload));
    /// # let _: Router = app;
    /// ```
    pub async fn chunk(&mut self) -> Result<Option<Bytes>, MultipartError> {
        self.inner
            .chunk()
            .await
            .map_err(MultipartError::from_multer)
    }
}

/// Errors associated with parsing `multipart/form-data` requests.
#[derive(Debug)]
pub struct MultipartError {
    source: multer::Error,
}

impl MultipartError {
    fn from_multer(multer: multer::Error) -> Self {
        Self { source: multer }
    }

    /// Get the response body text used for this rejection.
    pub fn body_text(&self) -> String {
        axum_core::__log_rejection!(
            rejection_type = Self,
            body_text = self.body_text(),
            status = self.status(),
        );
        self.source.to_string()
    }

    /// Get the status code used for this rejection.
    pub fn status(&self) -> http::StatusCode {
        status_code_from_multer_error(&self.source)
    }
}

fn status_code_from_multer_error(err: &multer::Error) -> StatusCode {
    match err {
        multer::Error::UnknownField { .. }
        | multer::Error::IncompleteFieldData { .. }
        | multer::Error::IncompleteHeaders
        | multer::Error::ReadHeaderFailed(..)
        | multer::Error::DecodeHeaderName { .. }
        | multer::Error::DecodeContentType(..)
        | multer::Error::NoBoundary
        | multer::Error::DecodeHeaderValue { .. }
        | multer::Error::NoMultipart
        | multer::Error::IncompleteStream => StatusCode::BAD_REQUEST,
        multer::Error::FieldSizeExceeded { .. } | multer::Error::StreamSizeExceeded { .. } => {
            StatusCode::PAYLOAD_TOO_LARGE
        }
        multer::Error::StreamReadFailed(err) => {
            if let Some(err) = err.downcast_ref::<multer::Error>() {
                return status_code_from_multer_error(err);
            }

            if err
                .downcast_ref::<axum::Error>()
                .and_then(|err| err.source())
                .and_then(|err| err.downcast_ref::<http_body::LengthLimitError>())
                .is_some()
            {
                return StatusCode::PAYLOAD_TOO_LARGE;
            }

            StatusCode::INTERNAL_SERVER_ERROR
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

impl IntoResponse for MultipartError {
    fn into_response(self) -> Response {
        (self.status(), self.body_text()).into_response()
    }
}

impl fmt::Display for MultipartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error parsing `multipart/form-data` request")
    }
}

impl std::error::Error for MultipartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

fn parse_boundary(headers: &HeaderMap) -> Option<String> {
    let content_type = headers.get(CONTENT_TYPE)?.to_str().ok()?;
    multer::parse_boundary(content_type).ok()
}

/// Rejection used for [`Multipart`].
///
/// Contains one variant for each way the [`Multipart`] extractor can fail.
#[derive(Debug)]
#[non_exhaustive]
pub enum MultipartRejection {
    #[allow(missing_docs)]
    InvalidBoundary(InvalidBoundary),
}

impl IntoResponse for MultipartRejection {
    fn into_response(self) -> Response {
        match self {
            Self::InvalidBoundary(inner) => inner.into_response(),
        }
    }
}

impl MultipartRejection {
    /// Get the response body text used for this rejection.
    pub fn body_text(&self) -> String {
        match self {
            Self::InvalidBoundary(inner) => inner.body_text(),
        }
    }

    /// Get the status code used for this rejection.
    pub fn status(&self) -> http::StatusCode {
        match self {
            Self::InvalidBoundary(inner) => inner.status(),
        }
    }
}

impl From<InvalidBoundary> for MultipartRejection {
    fn from(inner: InvalidBoundary) -> Self {
        Self::InvalidBoundary(inner)
    }
}

impl std::fmt::Display for MultipartRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBoundary(inner) => write!(f, "{}", inner.body_text()),
        }
    }
}

impl std::error::Error for MultipartRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidBoundary(inner) => Some(inner),
        }
    }
}

/// Rejection type used if the `boundary` in a `multipart/form-data` is
/// missing or invalid.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct InvalidBoundary;

impl IntoResponse for InvalidBoundary {
    fn into_response(self) -> Response {
        (self.status(), self.body_text()).into_response()
    }
}

impl InvalidBoundary {
    /// Get the response body text used for this rejection.
    pub fn body_text(&self) -> String {
        "Invalid `boundary` for `multipart/form-data` request".into()
    }

    /// Get the status code used for this rejection.
    pub fn status(&self) -> http::StatusCode {
        http::StatusCode::BAD_REQUEST
    }
}

impl std::fmt::Display for InvalidBoundary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.body_text())
    }
}

impl std::error::Error for InvalidBoundary {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{
        body::Body, extract::DefaultBodyLimit, response::IntoResponse, routing::post, Router,
    };

    #[tokio::test]
    async fn content_type_with_encoding() {
        const BYTES: &[u8] = "<!doctype html><title>🦀</title>".as_bytes();
        const FILE_NAME: &str = "index.html";
        const CONTENT_TYPE: &str = "text/html; charset=utf-8";

        async fn handle(mut multipart: Multipart) -> impl IntoResponse {
            let field = multipart.next_field().await.unwrap().unwrap();

            assert_eq!(field.file_name().unwrap(), FILE_NAME);
            assert_eq!(field.content_type().unwrap(), CONTENT_TYPE);
            assert_eq!(field.bytes().await.unwrap(), BYTES);

            assert!(multipart.next_field().await.unwrap().is_none());
        }

        let app = Router::new().route("/", post(handle));

        let client = TestClient::new(app);

        let form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(BYTES)
                .file_name(FILE_NAME)
                .mime_str(CONTENT_TYPE)
                .unwrap(),
        );

        client.post("/").multipart(form).send().await;
    }

    // No need for this to be a #[test], we just want to make sure it compiles
    fn _multipart_from_request_limited() {
        async fn handler(_: Multipart) {}
        let _app: Router<(), http_body::Limited<Body>> = Router::new().route("/", post(handler));
    }

    #[tokio::test]
    async fn body_too_large() {
        const BYTES: &[u8] = "<!doctype html><title>🦀</title>".as_bytes();

        async fn handle(mut multipart: Multipart) -> Result<(), MultipartError> {
            while let Some(field) = multipart.next_field().await? {
                field.bytes().await?;
            }
            Ok(())
        }

        let app = Router::new()
            .route("/", post(handle))
            .layer(DefaultBodyLimit::max(BYTES.len() - 1));

        let client = TestClient::new(app);

        let form =
            reqwest::multipart::Form::new().part("file", reqwest::multipart::Part::bytes(BYTES));

        let res = client.post("/").multipart(form).send().await;
        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
