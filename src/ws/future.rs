//! Future types.

use bytes::Bytes;
use http::{HeaderValue, Response, StatusCode};
use http_body::Full;
use sha1::{Digest, Sha1};
use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// Response future for [`WebSocketUpgrade`](super::WebSocketUpgrade).
#[derive(Debug)]
pub struct ResponseFuture(Result<Option<HeaderValue>, Option<(StatusCode, &'static str)>>);

impl ResponseFuture {
    pub(super) fn ok(key: HeaderValue) -> Self {
        Self(Ok(Some(key)))
    }

    pub(super) fn err(status: StatusCode, body: &'static str) -> Self {
        Self(Err(Some((status, body))))
    }
}

impl Future for ResponseFuture {
    type Output = Result<Response<Full<Bytes>>, Infallible>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = match self.get_mut().0.as_mut() {
            Ok(key) => Response::builder()
                .status(StatusCode::SWITCHING_PROTOCOLS)
                .header(
                    http::header::CONNECTION,
                    HeaderValue::from_str("upgrade").unwrap(),
                )
                .header(
                    http::header::UPGRADE,
                    HeaderValue::from_str("websocket").unwrap(),
                )
                .header(
                    http::header::SEC_WEBSOCKET_ACCEPT,
                    sign(key.take().unwrap().as_bytes()),
                )
                .body(Full::new(Bytes::new()))
                .unwrap(),
            Err(err) => {
                let (status, body) = err.take().unwrap();
                Response::builder()
                    .status(status)
                    .body(Full::from(body))
                    .unwrap()
            }
        };

        Poll::Ready(Ok(res))
    }
}

fn sign(key: &[u8]) -> HeaderValue {
    let mut sha1 = Sha1::default();
    sha1.update(key);
    sha1.update(&b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11"[..]);
    let b64 = Bytes::from(base64::encode(&sha1.finalize()));
    HeaderValue::from_maybe_shared(b64).expect("base64 is a valid value")
}
