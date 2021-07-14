//! Extractor that parses `multipart/form-data` requests commonly used with file upload forms.
//!
//! See [`Multipart`] for more details.

use super::{rejection::*, take_body, FromRequest};
use async_trait::async_trait;
use bytes::{Buf, Bytes, BytesMut};
use futures_util::{future::poll_fn, ready, stream::Stream};
use http::header::{HeaderMap, CONTENT_TYPE};
use http_body::Body;
use mime::Mime;
use std::{
    fmt::{self, Debug},
    pin::Pin,
    str,
    task::{Context, Poll},
};
use tower::BoxError;

/// Extractor that parses `multipart/form-data` requests commonly used with file upload forms.
///
/// Implementation is based on [RFC 7578](https://datatracker.ietf.org/doc/html/rfc7578).
///
/// # Example
///
/// ```rust,no_run
/// use axum::{prelude::*, extract::Multipart};
/// use bytes::BytesMut;
/// use futures::stream::StreamExt;
/// use http::StatusCode;
///
/// async fn file_upload(mut multipart: Multipart) -> Result<(), StatusCode> {
///     while let Some(part) = multipart.next_part().await {
///         let mut part = part.map_err(|_err| StatusCode::BAD_REQUEST)?;
///         println!("received part = {:?}", part);
///
///         // buffer data for part
///         let mut data = BytesMut::new();
///         while let Some(chunk) = part.next().await {
///             let chunk = chunk.map_err(|_err| StatusCode::BAD_REQUEST)?;
///             data.extend_from_slice(&chunk);
///         }
///         println!("file length = {} bytes", data.len());
///     }
///
///     Ok(())
/// }
///
/// let app = route("/upload", post(file_upload));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
// This could in theory be extracted into its down crate since all it needs is `http_body::Body`
pub struct Multipart<B = crate::body::Body> {
    /// The request body we're reading from
    body: B,
    /// Cached boundary
    middle_part_boundary: String,
    /// Cached boundary of final part
    final_part_boundary: String,
    /// Which part of a part are we currently decoding
    state: DecodeState,
    /// The buffer we're copying bytes into from the body to operation on them.
    buf: BytesMut,
    /// Headers we've parsed if we're unable to parse all headers from a part
    /// at once.
    headers: Vec<Header>,
    /// The content disposition of the part we're currently parsing
    content_disposition: Option<ContentDisposition>,
    /// The content type of the part we're currently parsing
    content_type: Option<Mime>,
    /// Have we reached the end of the body yet?
    ///
    /// `true` if polling data from the body returned `None`, meaning the body
    /// is empty and no more data will come
    end_of_stream: bool,
}

impl<B> fmt::Debug for Multipart<B>
where
    B: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Multipart")
            .field("body", &self.body)
            .finish()
    }
}

#[derive(PartialEq, Eq)]
enum DecodeState {
    Boundary,
    Headers,
    Data,
}

impl<B> Multipart<B>
where
    B: Body<Data = Bytes> + Unpin,
    B::Error: Into<BoxError>,
{
    /// Read the next "part" from the request.
    ///
    /// Returns `None` if the request body has been fully exhausted or if an error was previously
    /// encountered.
    pub async fn next_part(&mut self) -> Option<Result<Part<'_, B>, MultipartError>> {
        if self.state == DecodeState::Data {
            // A `Part` was given out but `Part::data` wasn't called before
            // `Multipart::next_part` was called again. So we have to skip over
            // the data of the discarded part.
            while let Some(chunk) = poll_fn(|cx| self.poll_data(cx)).await {
                if let Err(err) = chunk {
                    return Some(Err(err));
                }
            }
        }

        loop {
            if self.end_of_stream && self.buf.is_empty() {
                // no more data coming and we've consumed everything that we buffered
                return None;
            }

            match poll_fn(|cx| self.poll_body(cx)).await {
                Ok(()) => {}
                Err(err) => return Some(Err(err)),
            }

            match self.decode_part() {
                None => {}
                Some(Ok(())) => {
                    let content_disposition = if let Some(cd) = self.content_disposition.take() {
                        cd
                    } else {
                        return Some(Err(MultipartError::parse_error(
                            "Missing `Content-Disposition` header in part",
                        )));
                    };

                    let part = Part {
                        content_disposition,
                        content_type: self.content_type.take(),
                        multipart: self,
                    };

                    return Some(Ok(part));
                }
                Some(Err(err)) => {
                    self.end_of_stream = true;
                    self.buf.clear();
                    return Some(Err(err));
                }
            }
        }
    }

    /// Get the next chunk from `self.body` and append it to `self.buf`.
    fn poll_body(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), MultipartError>> {
        assert!(
            !self.end_of_stream,
            "`poll_data` was called after hitting end-of-stream"
        );

        match ready!(Pin::new(&mut self.body).poll_data(cx)) {
            None => {
                self.end_of_stream = true;
                Poll::Ready(Ok(()))
            }
            Some(Err(err)) => {
                self.end_of_stream = true;
                self.buf.clear();
                Poll::Ready(Err(MultipartError::body_error(err)))
            }
            Some(Ok(chunk)) => {
                self.buf.extend_from_slice(&chunk);
                Poll::Ready(Ok(()))
            }
        }
    }

    fn decode_part(&mut self) -> Option<Result<(), MultipartError>> {
        macro_rules! t {
            ($expr:expr) => {
                match $expr {
                    Ok(value) => value,
                    Err(err) => return Some(Err(err)),
                }
            };
        }

        loop {
            match self.state {
                DecodeState::Boundary => {
                    if self.buf.len() < self.middle_part_boundary.len() {
                        return None;
                    }

                    if let Some(pos) = self.seek_middle_boundary() {
                        self.buf.advance(pos + self.middle_part_boundary.len());
                        self.state = DecodeState::Headers;
                    } else if self.end_of_stream {
                        return Some(Err(MultipartError::parse_error("Invalid boundary")));
                    } else {
                        return None;
                    }
                }
                DecodeState::Headers => {
                    // keep reading until we've buffered all the headers
                    let end_of_headers = "\r\n\r\n";
                    if !self
                        .buf
                        .windows(end_of_headers.len())
                        .any(|window| window == end_of_headers.as_bytes())
                    {
                        return None;
                    }

                    let mut headers = [httparse::EMPTY_HEADER; 4];

                    let headers = match httparse::parse_headers(&self.buf, &mut headers) {
                        Ok(httparse::Status::Partial) => {
                            // should in theory never get in here since we buffer until
                            // we've reached `end_of_headers`
                            let new_headers = owned_headers_from_httparse_headers(&headers);
                            self.headers.extend(new_headers);
                            self.buf.clear();
                            return None;
                        }
                        Ok(httparse::Status::Complete((read, headers))) => {
                            let new_headers = owned_headers_from_httparse_headers(headers);
                            self.headers.extend(new_headers);
                            self.buf.advance(read);
                            self.take_headers()
                        }
                        Err(err) => {
                            return Some(Err(MultipartError::parse_error(&format!(
                                "Failed to parse a header: {}",
                                err
                            ))))
                        }
                    };

                    for header in headers {
                        if header.name.eq_ignore_ascii_case("content-disposition") {
                            self.content_disposition =
                                Some(t!(parse_content_disposition(&header.value)));
                        } else if header.name.eq_ignore_ascii_case("content-type") {
                            self.content_type = Some(t!(parse_content_type(&header.value)));
                        } else {
                            return Some(Err(MultipartError::parse_error(&format!(
                                "Unknown header in part: {}",
                                header.name
                            ))));
                        }
                    }

                    self.state = DecodeState::Data;
                    return Some(Ok(()));
                }
                DecodeState::Data => {
                    // `Multipart::next_part` makes sure we never get in here
                    unreachable!()
                }
            }
        }
    }

    fn poll_data(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<Bytes, MultipartError>>> {
        loop {
            if self.state != DecodeState::Data {
                return Poll::Ready(None);
            }

            if self.buf.is_empty() {
                if self.end_of_stream {
                    // no more data coming and we've consumed everything that we buffered
                    return Poll::Ready(None);
                } else {
                    match ready!(self.poll_body(cx)) {
                        Ok(()) => {}
                        Err(err) => return Poll::Ready(Some(Err(err))),
                    }
                    continue;
                }
            }

            // check if the buf contains the boundary
            if let Some(pos) = self.seek_middle_boundary() {
                let data = self.buf.split_to(pos - "\r\n".len()).freeze();

                self.buf.advance("\r\n".len());
                self.state = DecodeState::Boundary;

                return Poll::Ready(Some(Ok(data)));
            }

            if !self.end_of_stream && self.buf.ends_with(b"\r\n") {
                // we don't know if this is the end of the part or just a chunk
                // that happends to end with `\r\n` so we have to buffer more data
                // to see if we hit the boundary
                match ready!(self.poll_body(cx)) {
                    Ok(()) => {}
                    Err(err) => return Poll::Ready(Some(Err(err))),
                }
                continue;
            }

            if self.buf.ends_with(self.final_part_boundary.as_bytes()) {
                let data = self
                    .buf
                    .split_to(self.buf.len() - self.final_part_boundary.len() - "\r\n".len())
                    .freeze();
                self.buf.clear();
                self.end_of_stream = true;
                return Poll::Ready(Some(Ok(data)));
            } else {
                let data = self.buf.clone().freeze();
                self.buf.clear();
                return Poll::Ready(Some(Ok(data)));
            }
        }
    }

    fn seek_middle_boundary(&self) -> Option<usize> {
        self.buf
            .windows(self.middle_part_boundary.len())
            .position(|window| window == self.middle_part_boundary.as_bytes())
    }

    fn take_headers(&mut self) -> Vec<Header> {
        self.headers.drain(..).collect()
    }
}

/// A single part of a `multipart/form-data` request.
///
/// Implements [`Stream`] which allows asynchronously consuming data from the request body.
///
/// See [`Multipart`] for an example.
///
/// [`Stream`]: futures_util::stream::Stream
pub struct Part<'a, B = crate::body::Body> {
    content_disposition: ContentDisposition,
    content_type: Option<Mime>,
    multipart: &'a mut Multipart<B>,
}

impl<'a, B> Part<'a, B> {
    /// The "name" of the input field in the form this part is associated with.
    pub fn name(&self) -> &str {
        &self.content_disposition.name
    }

    /// The filename of this part, if any.
    pub fn filename(&self) -> Option<&str> {
        self.content_disposition.filename.as_deref()
    }

    /// The mime type of this part, if any.
    pub fn content_type(&self) -> Option<&Mime> {
        self.content_type.as_ref()
    }
}

impl<'a, B> Stream for Part<'a, B>
where
    B: Body<Data = Bytes> + Unpin,
    B::Error: Into<BoxError>,
{
    type Item = Result<Bytes, MultipartError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.multipart.poll_data(cx)
    }
}

impl<'a, B> fmt::Debug for Part<'a, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Part")
            .field("name", &self.content_disposition.name)
            .field("filename", &self.content_disposition.filename)
            .field("content_type", &self.content_type)
            .finish()
    }
}

#[derive(Debug)]
struct Header {
    name: String,
    value: Bytes,
}

fn owned_headers_from_httparse_headers(headers: &[httparse::Header<'_>]) -> Vec<Header> {
    headers
        .iter()
        .filter(|header| **header != httparse::EMPTY_HEADER)
        .map(|header| {
            let name = header.name.to_string();
            let value = Bytes::copy_from_slice(header.value);
            Header { name, value }
        })
        .collect::<Vec<_>>()
}

/// Error associated with consuming `multipart/form-data` requests.
#[derive(Debug)]
pub struct MultipartError {
    kind: ErrorKind,
}

#[derive(Debug)]
enum ErrorKind {
    ParseError(String),
    BodyError(BoxError),
    ParseMime(BoxError),
    UnsupportedMime(Mime),
}

impl MultipartError {
    fn parse_error(s: &str) -> Self {
        Self {
            kind: ErrorKind::ParseError(s.to_string()),
        }
    }

    fn body_error<E>(err: E) -> Self
    where
        E: Into<BoxError>,
    {
        Self {
            kind: ErrorKind::BodyError(err.into()),
        }
    }

    fn parse_mime<E>(err: E) -> Self
    where
        E: Into<BoxError>,
    {
        Self {
            kind: ErrorKind::ParseMime(err.into()),
        }
    }
}

impl fmt::Display for MultipartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::ParseError(inner) => write!(f, "Parse error: {}", inner),
            ErrorKind::BodyError(inner) => write!(f, "Body error: {}", inner),
            ErrorKind::ParseMime(inner) => write!(f, "Error parsing mime type: {}", inner),
            ErrorKind::UnsupportedMime(mime) => write!(f, "Mime type `{}` is not supported", mime),
        }
    }
}

impl std::error::Error for MultipartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::ParseError(_) => None,
            ErrorKind::BodyError(inner) => Some(&**inner),
            ErrorKind::ParseMime(inner) => Some(&**inner),
            ErrorKind::UnsupportedMime(_) => None,
        }
    }
}

fn parse_content_disposition(header_value: &[u8]) -> Result<ContentDisposition, MultipartError> {
    let header_value = std::str::from_utf8(header_value).unwrap();
    let header_value = format!("multipart/{}", header_value);

    let mime = if let Ok(mime) = header_value.parse::<Mime>() {
        mime
    } else {
        return Err(MultipartError::parse_error(
            "Failed to parse `Content-Disposition` header",
        ));
    };

    let name = mime
        .get_param("name")
        .map(|name| name.as_str().to_string())
        .ok_or_else(|| {
            MultipartError::parse_error("Missing `name` parameter in `Content-Disposition` header")
        })?;

    let filename = mime
        .get_param("filename")
        .map(|name| name.as_str().to_string());

    Ok(ContentDisposition { name, filename })
}

fn parse_content_type(header_value: &[u8]) -> Result<Mime, MultipartError> {
    let s = str::from_utf8(header_value).map_err(MultipartError::parse_mime)?;
    let mime = s.parse::<Mime>().map_err(MultipartError::parse_mime)?;

    if mime.subtype() == "multipart/mixed" {
        Err(MultipartError {
            kind: ErrorKind::UnsupportedMime(mime),
        })
    } else {
        Ok(mime)
    }
}

#[derive(Debug)]
struct ContentDisposition {
    name: String,
    filename: Option<String>,
}

#[async_trait]
impl<B> FromRequest<B> for Multipart<B>
where
    B: Default + Send,
{
    type Rejection = MultipartRejection;

    async fn from_request(req: &mut http::Request<B>) -> Result<Self, Self::Rejection> {
        if req.method() != http::Method::POST {
            return Err(MethodMustBePost.into());
        }

        let body = take_body(req)?;
        let mime = mime_from_content_type(req.headers())?;
        let boundary = boundary_from_mime(&mime)?;

        Ok(Multipart {
            body,
            // the cached boundaries don't contain the leading `\r\n` because
            // the first part doesn't start with that
            middle_part_boundary: format!("--{}\r\n", boundary),
            final_part_boundary: format!("--{}--\r\n", boundary),
            state: DecodeState::Boundary,
            buf: BytesMut::new(),
            headers: Vec::new(),
            content_disposition: None,
            content_type: None,
            end_of_stream: false,
        })
    }
}

fn mime_from_content_type(headers: &HeaderMap) -> Result<Mime, InvalidMultipartContentType> {
    let content_type = if let Some(content_type) = headers.get(CONTENT_TYPE) {
        content_type
            .to_str()
            .map_err(InvalidMultipartContentType::from_err)?
    } else {
        return Err(InvalidMultipartContentType::from_err(
            "`Content-Type` wasn't `multipart/form-data`",
        ));
    };

    let mime = content_type
        .parse::<Mime>()
        .map_err(InvalidMultipartContentType::from_err)?;

    if mime.essence_str() != "multipart/form-data" {
        return Err(InvalidMultipartContentType::from_err(
            "`Content-Type` wasn't `multipart/form-data`",
        ));
    }

    Ok(mime)
}

fn boundary_from_mime(mime: &Mime) -> Result<&str, InvalidMultipartContentType> {
    let boundary = mime
        .get_param(mime::BOUNDARY)
        .ok_or_else(|| {
            InvalidMultipartContentType::from_err(BoxError::from("Missing `boundary` param"))
        })?
        .as_str();

    Ok(boundary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::prelude::*;
    use http::{Method, Request};

    #[tokio::test]
    async fn basic() {
        let stream = futures_util::stream::iter(vec![
            Ok::<_, hyper::Error>("------WebKitFormBoundaryDtlRiQpz99walEeV\r\n"),
            Ok("Content-Disposition: form-data; name=\"first name\"\r\n"),
            Ok("\r\n"),
            Ok("B"),
            Ok("o"),
            Ok("b\r\n"),
            Ok("------WebKitFormBoundaryDtlRiQpz99walEeV\r\n"),
            Ok("Content-Disposition: form-data; name=\"file\"; filename=\"small-file\"\r\n"),
            Ok("Content-Type: application/octet-stream\r\n"),
            Ok("\r\n"),
            Ok("Hello, World!\n\r\n"),
            Ok("------WebKitFormBoundaryDtlRiQpz99walEeV--\r\n"),
        ]);
        let body = hyper::Body::wrap_stream(stream);

        let mut request = Request::builder()
            .header(
                "content-type",
                "multipart/form-data; boundary=----WebKitFormBoundaryDtlRiQpz99walEeV",
            )
            .method(Method::POST)
            .body(body)
            .unwrap();

        let mut multipart = Multipart::from_request(&mut request).await.unwrap();

        // first part
        let mut first_name_part = multipart.next_part().await.unwrap().unwrap();

        assert_eq!(first_name_part.name(), "first name");
        assert!(first_name_part.filename().is_none());

        let mut data = BytesMut::new();
        while let Some(chunk) = first_name_part.next().await {
            data.extend_from_slice(&chunk.unwrap());
        }
        assert_eq!(str::from_utf8(&data[..]).unwrap(), "Bob");

        // second part
        let mut file_part = multipart.next_part().await.unwrap().unwrap();
        assert_eq!(file_part.name(), "file");
        assert_eq!(file_part.filename().unwrap(), "small-file");
        assert_eq!(
            file_part.content_type().unwrap(),
            &"application/octet-stream"
        );

        let mut data = BytesMut::new();
        while let Some(chunk) = file_part.next().await {
            data.extend_from_slice(&chunk.unwrap());
        }
        assert_eq!(str::from_utf8(&data[..]).unwrap(), "Hello, World!\n");

        // no more parts left
        let file_part = multipart.next_part().await;
        assert!(matches!(dbg!(file_part), None));
    }

    #[tokio::test]
    async fn getting_the_next_part_without_consuming_data() {
        let stream = futures_util::stream::iter(vec![
            Ok::<_, hyper::Error>("------WebKitFormBoundaryDtlRiQpz99walEeV\r\n"),
            Ok("Content-Disposition: form-data; name=\"first name\"\r\n"),
            Ok("\r\n"),
            Ok("Bob\r\n"),
            Ok("------WebKitFormBoundaryDtlRiQpz99walEeV\r\n"),
            Ok("Content-Disposition: form-data; name=\"file\"; filename=\"small-file\"\r\n"),
            Ok("Content-Type: application/octet-stream\r\n"),
            Ok("\r\n"),
            Ok("Hello, World!\n\r\n"),
            Ok("------WebKitFormBoundaryDtlRiQpz99walEeV--\r\n"),
        ]);
        let body = hyper::Body::wrap_stream(stream);

        let mut request = Request::builder()
            .header(
                "content-type",
                "multipart/form-data; boundary=----WebKitFormBoundaryDtlRiQpz99walEeV",
            )
            .method(Method::POST)
            .body(body)
            .unwrap();

        let mut multipart = Multipart::from_request(&mut request).await.unwrap();

        // first part
        let first_name_part = multipart.next_part().await.unwrap().unwrap();

        assert_eq!(first_name_part.name(), "first name");
        assert!(first_name_part.filename().is_none());

        // second part
        let file_part = multipart.next_part().await.unwrap().unwrap();
        assert_eq!(file_part.name(), "file");
        assert_eq!(file_part.filename().unwrap(), "small-file");

        // no more parts left
        let file_part = multipart.next_part().await;
        assert!(matches!(dbg!(file_part), None));
    }

    #[tokio::test]
    async fn sending_everything_as_one_chunk() {
        let body = vec![
            "------WebKitFormBoundaryDtlRiQpz99walEeV\r\n",
            "Content-Disposition: form-data; name=\"first name\"\r\n",
            "\r\n",
            "Bob\r\n",
            "------WebKitFormBoundaryDtlRiQpz99walEeV\r\n",
            "Content-Disposition: form-data; name=\"file\"; filename=\"small-file\"\r\n",
            "Content-Type: application/octet-stream\r\n",
            "\r\n",
            "Hello, World!\n\r\n",
            "------WebKitFormBoundaryDtlRiQpz99walEeV--\r\n",
        ]
        .into_iter()
        .collect::<String>();
        let stream = futures_util::stream::iter(vec![Ok::<_, hyper::Error>(body)]);
        let body = hyper::Body::wrap_stream(stream);

        let mut request = Request::builder()
            .header(
                "content-type",
                "multipart/form-data; boundary=----WebKitFormBoundaryDtlRiQpz99walEeV",
            )
            .method(Method::POST)
            .body(body)
            .unwrap();

        let mut multipart = Multipart::from_request(&mut request).await.unwrap();

        // first part
        let mut first_name_part = multipart.next_part().await.unwrap().unwrap();

        assert_eq!(first_name_part.name(), "first name");
        assert!(first_name_part.filename().is_none());

        let mut data = BytesMut::new();
        while let Some(chunk) = first_name_part.next().await {
            data.extend_from_slice(&chunk.unwrap());
        }
        assert_eq!(str::from_utf8(&data[..]).unwrap(), "Bob");

        // second part
        let mut file_part = multipart.next_part().await.unwrap().unwrap();
        assert_eq!(file_part.name(), "file");
        assert_eq!(file_part.filename().unwrap(), "small-file");
        assert_eq!(
            file_part.content_type().unwrap(),
            &"application/octet-stream"
        );

        let mut data = BytesMut::new();
        while let Some(chunk) = file_part.next().await {
            data.extend_from_slice(&chunk.unwrap());
        }
        assert_eq!(str::from_utf8(&data[..]).unwrap(), "Hello, World!\n");

        // no more parts left
        let file_part = multipart.next_part().await;
        assert!(matches!(dbg!(file_part), None));
    }

    #[tokio::test]
    async fn body_error() {
        let stream = futures_util::stream::iter(vec![
            Ok::<_, BoxError>("------WebKitFormBoundaryDtlRiQpz99walEeV\r\n"),
            Ok("Content-Disposition: form-data; name=\"first name\"\r\n"),
            Ok("\r\n"),
            Ok("Bob\r\n"),
            Ok("------WebKitFormBoundaryDtlRiQpz99walEeV\r\n"),
            Err("body error".into()),
        ]);
        let body = hyper::Body::wrap_stream(stream);

        let mut request = Request::builder()
            .header(
                "content-type",
                "multipart/form-data; boundary=----WebKitFormBoundaryDtlRiQpz99walEeV",
            )
            .method(Method::POST)
            .body(body)
            .unwrap();

        let mut multipart = Multipart::from_request(&mut request).await.unwrap();

        // first part
        let mut first_name_part = multipart.next_part().await.unwrap().unwrap();

        assert_eq!(first_name_part.name(), "first name");
        assert!(first_name_part.filename().is_none());

        let mut data = BytesMut::new();
        while let Some(chunk) = first_name_part.next().await {
            data.extend_from_slice(&chunk.unwrap());
        }
        assert_eq!(str::from_utf8(&data[..]).unwrap(), "Bob");

        // second part should be a body error
        let error = multipart.next_part().await.unwrap().unwrap_err();
        error.kind.into_body_error();

        // no more parts left
        let file_part = multipart.next_part().await;
        assert!(matches!(dbg!(file_part), None));
    }

    #[test]
    fn test_parse_content_disposition() {
        let header_value: &[u8] = b"form-data; name=\"first name\"";

        let cd = parse_content_disposition(header_value).unwrap();

        assert_eq!(cd.name, "first name");
        assert_eq!(cd.filename, None);
    }

    #[test]
    fn test_parse_content_disposition_with_filename() {
        let header_value: &[u8] = b"form-data; name=\"file\"; filename=\"small-file\"";
        let cd = parse_content_disposition(header_value).unwrap();

        assert_eq!(cd.name, "file");
        assert_eq!(cd.filename.unwrap(), "small-file");
    }

    #[test]
    fn test_parse_content_disposition_too_long() {
        let header_value: &[u8] = b"form-data; name=\"first name\" foobar";

        assert_eq!(
            parse_content_disposition(header_value)
                .unwrap_err()
                .kind
                .into_parse_error(),
            "Failed to parse `Content-Disposition` header",
        );
    }

    #[test]
    fn test_parse_content_disposition_missing_name() {
        let header_value: &[u8] = b"form-data; filename=\"small-file\"";
        assert_eq!(
            parse_content_disposition(header_value)
                .unwrap_err()
                .kind
                .into_parse_error(),
            "Missing `name` parameter in `Content-Disposition` header",
        );
    }

    #[test]
    fn test_parse_content_disposition_missing_form_data() {
        let header_value: &[u8] = b"name=\"first name\"";
        assert_eq!(
            parse_content_disposition(header_value)
                .unwrap_err()
                .kind
                .into_parse_error(),
            "Failed to parse `Content-Disposition` header",
        );
    }

    // some test helpers
    impl ErrorKind {
        fn into_parse_error(self) -> String {
            if let ErrorKind::ParseError(s) = self {
                s
            } else {
                panic!()
            }
        }

        fn into_body_error(self) -> BoxError {
            if let ErrorKind::BodyError(err) = self {
                err
            } else {
                panic!()
            }
        }
    }
}
