//! Extractor that parses `multipart/form-data` requests commonly used with file uploads.
//!
//! See [`Multipart`] for more details.

use super::{rejection::*, BodyStream, FromRequest, RequestParts};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::Stream;
use http::header::{HeaderMap, CONTENT_TYPE};
use mime::Mime;
use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};
use tower::BoxError;

/// Extractor that parses `multipart/form-data` requests commonly used with file uploads.
///
/// # Example
///
/// ```rust,no_run
/// use axum::prelude::*;
/// use futures::stream::StreamExt;
///
/// async fn upload(mut multipart: extract::Multipart) {
///     while let Some(mut field) = multipart.next_field().await.unwrap() {
///         let name = field.name().unwrap().to_string();
///         let data = field.bytes().await.unwrap();
///
///         println!("Length of `{}` is {} bytes", name, data.len());
///     }
/// }
///
/// let app = route("/upload", post(upload));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// For security reasons its recommended to combine this with
/// [`ContentLengthLimit`](super::ContentLengthLimit) to limit the size of the request payload.
#[derive(Debug)]
pub struct Multipart {
    inner: multer::Multipart<'static>,
}

#[async_trait]
impl<B> FromRequest<B> for Multipart
where
    B: http_body::Body<Data = Bytes> + Default + Unpin + Send + 'static,
    B::Error: Into<BoxError> + 'static,
{
    type Rejection = MultipartRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let stream = BodyStream::from_request(req).await?;
        let headers = req.headers().ok_or(HeadersAlreadyExtracted)?;
        let boundary = parse_boundary(headers).ok_or(InvalidBoundary)?;
        let multipart = multer::Multipart::new(stream, boundary);
        Ok(Self { inner: multipart })
    }
}

impl Multipart {
    /// Yields the next [`Field`] if available.
    pub async fn next_field(&mut self) -> Result<Option<Field<'_>>, MultipartError> {
        let field = self
            .inner
            .next_field()
            .await
            .map_err(MultipartError::from_multer)?;

        if let Some(field) = field {
            Ok(Some(Field {
                inner: field,
                _multipart: self,
            }))
        } else {
            Ok(None)
        }
    }
}

/// A single field in a multipart stream.
#[derive(Debug)]
pub struct Field<'a> {
    inner: multer::Field<'static>,
    // multer requires there to only be one live `multer::Field` at any point. This enforces that
    // statically, which multer does not do, it returns an error instead.
    _multipart: &'a mut Multipart,
}

impl<'a> Stream for Field<'a> {
    type Item = Result<Bytes, MultipartError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner)
            .poll_next(cx)
            .map_err(MultipartError::from_multer)
    }
}

impl<'a> Field<'a> {
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

    /// Get the content type of the field.
    pub fn content_type(&self) -> Option<&Mime> {
        self.inner.content_type()
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

composite_rejection! {
    /// Rejection used for [`Multipart`].
    ///
    /// Contains one variant for each way the [`Multipart`] extractor can fail.
    pub enum MultipartRejection {
        BodyAlreadyExtracted,
        InvalidBoundary,
        HeadersAlreadyExtracted,
    }
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Invalid `boundary` for `multipart/form-data` request"]
    /// Rejection type used if the `boundary` in a `multipart/form-data` is
    /// missing or invalid.
    pub struct InvalidBoundary;
}
