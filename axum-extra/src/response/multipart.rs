//! Multipart framing shared by buffered forms and streamed byte ranges.

use axum_core::{
    body::{Body, BodyDataStream},
    Error,
};
use bytes::Bytes;
use futures_core::TryStream;
use futures_util::{stream, TryStreamExt};

/// A prepared multipart section with buffered framing and a streamed body.
pub(super) struct Part {
    /// Opening boundary, headers, and blank line; excludes the body and its trailing CRLF.
    pub(super) prefix: Bytes,
    /// Content emitted after the prefix, without buffering it in the encoder.
    body: Body,
}

impl Part {
    /// Build a section prefix from the boundary and headers, leaving `body` untouched.
    ///
    /// Callers must ensure the boundary and header names and values contain no line breaks.
    pub(super) fn new<'a>(
        boundary: &str,
        headers: impl IntoIterator<Item = (&'a str, &'a str)>,
        body: Body,
    ) -> Self {
        let headers = headers.into_iter();
        // Optimistic guess: fixed framing plus about 48 bytes per header; longer values may grow it.
        let mut prefix = Vec::with_capacity(boundary.len() + 6 + headers.size_hint().0 * 48);
        prefix.extend_from_slice(b"--");
        prefix.extend_from_slice(boundary.as_bytes());
        prefix.extend_from_slice(b"\r\n");
        for (name, value) in headers {
            prefix.extend_from_slice(name.as_bytes());
            prefix.extend_from_slice(b": ");
            prefix.extend_from_slice(value.as_bytes());
            prefix.extend_from_slice(b"\r\n");
        }
        prefix.extend_from_slice(b"\r\n");

        Self {
            prefix: prefix.into(),
            body,
        }
    }
}

/// Stream each part's prefix, body, and trailing CRLF, then the closing boundary.
///
/// Body read errors propagate through the returned stream. `trailing_crlf` controls whether
/// the closing boundary also ends in CRLF: file ranges include it, while forms omit it.
pub(super) fn encode<I>(
    boundary: String,
    parts: I,
    trailing_crlf: bool,
) -> impl TryStream<Ok = Bytes, Error = Error> + Send + 'static
where
    I: Iterator<Item = Part> + Send + 'static,
{
    stream::try_unfold(
        (boundary, parts, None::<BodyDataStream>, false),
        move |(boundary, mut parts, current, done)| async move {
            if done {
                return Ok(None);
            }

            if let Some(mut body) = current {
                return match body.try_next().await? {
                    Some(bytes) => Ok(Some((bytes, (boundary, parts, Some(body), false)))),
                    None => Ok(Some((
                        Bytes::from_static(b"\r\n"),
                        (boundary, parts, None, false),
                    ))),
                };
            }

            if let Some(part) = parts.next() {
                return Ok(Some((
                    part.prefix,
                    (boundary, parts, Some(part.body.into_data_stream()), false),
                )));
            }

            let ending = if trailing_crlf { "\r\n" } else { "" };
            let closing = Bytes::from(format!("--{boundary}--{ending}"));
            Ok::<_, Error>(Some((closing, (boundary, parts, None, true))))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{encode, Part};
    use axum_core::body::Body;
    use bytes::Bytes;
    use futures_util::stream;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn streams_body_chunks_in_order() {
        let chunks = stream::iter([
            Ok::<_, std::io::Error>(Bytes::from_static(b"ab")),
            Ok(Bytes::from_static(b"cd")),
        ]);
        let part = Part::new(
            "test",
            [("Content-Type", "text/plain")],
            Body::from_stream(chunks),
        );
        let body = Body::from_stream(encode("test".into(), [part].into_iter(), false));

        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(
            bytes.as_ref(),
            b"--test\r\nContent-Type: text/plain\r\n\r\nabcd\r\n--test--"
        );
    }
}
