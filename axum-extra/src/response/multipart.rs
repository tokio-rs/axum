//! Multipart framing shared by buffered forms and streamed byte ranges.

use axum_core::{body::Body, Error};
use bytes::Bytes;
use http_body::{Body as HttpBody, Frame, SizeHint};
use std::{
    pin::Pin,
    task::{ready, Context, Poll},
};

const CRLF: &[u8] = b"\r\n";

/// A prepared multipart section with buffered framing and a streamed body.
pub(super) struct Part {
    /// Opening boundary, headers, and blank line; excludes the body and its trailing CRLF.
    prefix: Bytes,
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

    /// Exact remaining length, including framing, if the body has an exact size hint.
    fn content_length(&self) -> Option<u64> {
        (self.prefix.len() as u64)
            .checked_add(self.body.size_hint().exact()?)?
            .checked_add(CRLF.len() as u64)
    }
}

/// A streaming multipart body that derives its size hint from its remaining parts.
///
/// Storage can be a vector for forms or a fixed-size array for file ranges. Consumed parts
/// are cleared, and unknown or overflowing lengths never produce an exact size hint.
pub(super) struct Multipart<P> {
    parts: P,
    index: usize,
    closing: Bytes,
}

impl<P> Multipart<P> {
    /// Build the closing boundary without reading any part bodies.
    ///
    /// File ranges include a final CRLF; forms omit it to preserve their wire format.
    pub(super) fn new(boundary: &str, parts: P, trailing_crlf: bool) -> Self {
        let ending = if trailing_crlf { "\r\n" } else { "" };
        Self {
            parts,
            index: 0,
            closing: Bytes::from(format!("--{boundary}--{ending}")),
        }
    }
}

impl<P> HttpBody for Multipart<P>
where
    P: AsRef<[Option<Part>]> + AsMut<[Option<Part>]> + Unpin,
{
    type Data = Bytes;
    type Error = Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        loop {
            let Some(slot) = this.parts.as_mut().get_mut(this.index) else {
                return Poll::Ready(if this.closing.is_empty() {
                    None
                } else {
                    Some(Ok(Frame::data(std::mem::take(&mut this.closing))))
                });
            };
            let Some(part) = slot else {
                this.index += 1;
                continue;
            };
            if !part.prefix.is_empty() {
                return Poll::Ready(Some(Ok(Frame::data(std::mem::take(&mut part.prefix)))));
            }

            match ready!(Pin::new(&mut part.body).poll_frame(cx)) {
                Some(Ok(frame)) => {
                    // Part trailers are not multipart data, matching the previous data stream.
                    if let Ok(data) = frame.into_data() {
                        return Poll::Ready(Some(Ok(Frame::data(data))));
                    }
                }
                Some(Err(error)) => {
                    this.index = this.parts.as_ref().len();
                    this.closing = Bytes::new();
                    return Poll::Ready(Some(Err(error)));
                }
                None => {
                    *slot = None;
                    this.index += 1;
                    return Poll::Ready(Some(Ok(Frame::data(Bytes::from_static(CRLF)))));
                }
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.index == self.parts.as_ref().len() && self.closing.is_empty()
    }

    fn size_hint(&self) -> SizeHint {
        let length = self.parts.as_ref()[self.index..]
            .iter()
            .flatten()
            .try_fold(self.closing.len() as u64, |length, part| {
                length.checked_add(part.content_length()?)
            });
        length.map_or_else(SizeHint::new, SizeHint::with_exact)
    }
}

#[cfg(test)]
mod tests {
    use super::{Multipart, Part};
    use axum_core::body::Body;
    use bytes::Bytes;
    use futures_util::{stream, StreamExt};
    use http_body::{Body as _, Frame, SizeHint};
    use http_body_util::{BodyExt, Full};
    use std::{
        convert::Infallible,
        pin::Pin,
        task::{Context, Poll},
    };

    #[tokio::test]
    async fn exact_size_tracks_remaining_framing_and_data() {
        let cases: &[(&[&[u8]], &[u8])] = &[
            (&[], b"--test--"),
            (&[b""], b"--test\r\n\r\n\r\n--test--"),
            (
                &[b"a", b"bc"],
                b"--test\r\n\r\na\r\n--test\r\n\r\nbc\r\n--test--",
            ),
        ];
        for &(contents, expected) in cases {
            for trailing_crlf in [false, true] {
                let parts: Vec<_> = contents
                    .iter()
                    .map(|contents| {
                        let body = Full::new(Bytes::copy_from_slice(contents))
                            .with_trailers(async { Some(Ok(http::HeaderMap::new())) });
                        Some(Part::new("test", [], Body::new(body)))
                    })
                    .collect();
                let mut body = Multipart::new("test", parts, trailing_crlf);
                let mut expected = expected.to_vec();
                if trailing_crlf {
                    expected.extend_from_slice(b"\r\n");
                }
                let mut remaining = expected.len() as u64;
                let mut actual = Vec::new();
                assert_eq!(body.size_hint().exact(), Some(remaining));
                while let Some(frame) = body.frame().await {
                    let data = frame.unwrap().into_data().unwrap();
                    remaining -= data.len() as u64;
                    actual.extend_from_slice(&data);
                    assert_eq!(body.size_hint().exact(), Some(remaining));
                    assert_eq!(body.is_end_stream(), remaining == 0);
                }
                assert_eq!(actual, expected);
                assert!(body.is_end_stream());
            }
        }
    }

    #[tokio::test]
    async fn streams_body_chunks_in_order() {
        let chunks = stream::iter([
            Ok::<_, std::io::Error>(Bytes::from_static(b"ab")),
            Ok(Bytes::from_static(b"cd")),
        ])
        .then(|chunk| async move {
            tokio::task::yield_now().await;
            chunk
        });
        let part = Part::new(
            "test",
            [("Content-Type", "text/plain")],
            Body::from_stream(chunks),
        );
        let mut body = Multipart::new("test", [Some(part), None], false);
        assert_eq!(body.size_hint().exact(), None);

        let bytes = (&mut body).collect().await.unwrap().to_bytes();
        assert_eq!(
            bytes.as_ref(),
            b"--test\r\nContent-Type: text/plain\r\n\r\nabcd\r\n--test--"
        );
        assert_eq!(body.size_hint().exact(), Some(0));
        assert!(body.is_end_stream());
    }

    #[tokio::test]
    async fn body_error_stops_before_further_framing() {
        let chunks = stream::iter([
            Ok(Bytes::from_static(b"ab")),
            Err(std::io::Error::other("read failed")),
        ]);
        let parts = [
            Some(Part::new("test", [], Body::from_stream(chunks))),
            Some(Part::new("test", [], Body::from("not sent"))),
        ];
        let mut body = Multipart::new("test", parts, true);
        assert!(body.frame().await.unwrap().is_ok()); // Prefix.
        assert!(body.frame().await.unwrap().is_ok()); // Data.
        assert!(body.frame().await.unwrap().is_err());
        assert!(body.is_end_stream());
        assert_eq!(body.size_hint().exact(), Some(0));
        assert!(body.frame().await.is_none());
    }

    #[test]
    fn overflowing_lengths_have_no_exact_hint() {
        struct PendingBody(u64);

        impl http_body::Body for PendingBody {
            type Data = Bytes;
            type Error = Infallible;

            fn poll_frame(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
            ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
                Poll::Pending
            }

            fn size_hint(&self) -> SizeHint {
                SizeHint::with_exact(self.0)
            }
        }

        for length in [u64::MAX, u64::MAX / 2] {
            let parts = std::array::from_fn::<_, 2, _>(|_| {
                Some(Part::new("test", [], Body::new(PendingBody(length))))
            });
            let body = Multipart::new("test", parts, false);
            assert_eq!(body.size_hint().exact(), None);
        }
    }
}
