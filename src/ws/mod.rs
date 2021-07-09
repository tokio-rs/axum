//! Handle websocket connections.
//!
//! # Example
//!
//! ```
//! use axum::{prelude::*, ws::{ws, WebSocket}};
//!
//! let app = route("/ws", ws(handle_socket));
//!
//! async fn handle_socket(mut socket: WebSocket) {
//!     while let Some(msg) = socket.recv().await {
//!         let msg = msg.unwrap();
//!         socket.send(msg).await.unwrap();
//!     }
//! }
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```

use crate::{
    routing::EmptyRouter,
    service::{BoxResponseBody, OnMethod},
};
use bytes::Bytes;
use future::ResponseFuture;
use futures_util::{sink::SinkExt, stream::StreamExt};
use http::{
    header::{self, HeaderName},
    HeaderValue, Request, Response, StatusCode,
};
use http_body::Full;
use hyper::upgrade::{OnUpgrade, Upgraded};
use std::{borrow::Cow, convert::Infallible, fmt, future::Future, task::Context, task::Poll};
use tokio_tungstenite::{
    tungstenite::protocol::{self, WebSocketConfig},
    WebSocketStream,
};
use tower::{BoxError, Service};

pub mod future;

/// Create a new [`WebSocketUpgrade`] service that will call the closure with
/// each connection.
///
/// See the [module docs](crate::ws) for more details.
pub fn ws<F, Fut, B>(callback: F) -> OnMethod<BoxResponseBody<WebSocketUpgrade<F>, B>, EmptyRouter>
where
    F: FnOnce(WebSocket) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let svc = WebSocketUpgrade {
        callback,
        config: WebSocketConfig::default(),
    };
    crate::service::get::<_, B>(svc)
}

/// [`Service`] that ugprades connections to websockets and spawns a task to
/// handle the stream.
///
/// Created with [`ws`].
///
/// See the [module docs](crate::ws) for more details.
#[derive(Clone)]
pub struct WebSocketUpgrade<F> {
    callback: F,
    config: WebSocketConfig,
}

impl<F> fmt::Debug for WebSocketUpgrade<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebSocketUpgrade")
            .field("callback", &format_args!("{}", std::any::type_name::<F>()))
            .field("config", &self.config)
            .finish()
    }
}

impl<F> WebSocketUpgrade<F> {
    /// Set the size of the internal message send queue.
    pub fn max_send_queue(mut self, max: usize) -> Self {
        self.config.max_send_queue = Some(max);
        self
    }

    /// Set the maximum message size (defaults to 64 megabytes)
    pub fn max_message_size(mut self, max: usize) -> Self {
        self.config.max_message_size = Some(max);
        self
    }

    /// Set the maximum frame size (defaults to 16 megabytes)
    pub fn max_frame_size(mut self, max: usize) -> Self {
        self.config.max_frame_size = Some(max);
        self
    }
}

impl<ReqBody, F, Fut> Service<Request<ReqBody>> for WebSocketUpgrade<F>
where
    F: FnOnce(WebSocket) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    type Response = Response<Full<Bytes>>;
    type Error = Infallible;
    type Future = ResponseFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        if !header_eq(
            &req,
            header::CONNECTION,
            HeaderValue::from_static("upgrade"),
        ) {
            return ResponseFuture::err(
                StatusCode::BAD_REQUEST,
                "Connection header did not include 'upgrade'",
            );
        }

        if !header_eq(&req, header::UPGRADE, HeaderValue::from_static("websocket")) {
            return ResponseFuture::err(
                StatusCode::BAD_REQUEST,
                "`Upgrade` header did not include 'websocket'",
            );
        }

        if !header_eq(
            &req,
            header::SEC_WEBSOCKET_VERSION,
            HeaderValue::from_static("13"),
        ) {
            return ResponseFuture::err(
                StatusCode::BAD_REQUEST,
                "`Sec-Websocket-Version` header did not include '13'",
            );
        }

        let key = if let Some(key) = req.headers_mut().remove(header::SEC_WEBSOCKET_KEY) {
            key
        } else {
            return ResponseFuture::err(
                StatusCode::BAD_REQUEST,
                "`Sec-Websocket-Key` header missing",
            );
        };

        let on_upgrade = req.extensions_mut().remove::<OnUpgrade>().unwrap();

        let config = self.config;
        let callback = self.callback.clone();

        tokio::spawn(async move {
            let upgraded = on_upgrade.await.unwrap();
            let socket =
                WebSocketStream::from_raw_socket(upgraded, protocol::Role::Server, Some(config))
                    .await;
            let socket = WebSocket { inner: socket };
            callback(socket).await;
        });

        ResponseFuture::ok(key)
    }
}

fn header_eq<B>(req: &Request<B>, key: HeaderName, value: HeaderValue) -> bool {
    if let Some(header) = req.headers().get(&key) {
        header.as_bytes().eq_ignore_ascii_case(value.as_bytes())
    } else {
        false
    }
}

/// A stream of websocket messages.
#[derive(Debug)]
pub struct WebSocket {
    inner: WebSocketStream<Upgraded>,
}

impl WebSocket {
    /// Receive another message.
    ///
    /// Returns `None` is stream has closed.
    pub async fn recv(&mut self) -> Option<Result<Message, BoxError>> {
        self.inner
            .next()
            .await
            .map(|result| result.map_err(Into::into).map(|inner| Message { inner }))
    }

    /// Send a message.
    pub async fn send(&mut self, msg: Message) -> Result<(), BoxError> {
        self.inner.send(msg.inner).await.map_err(Into::into)
    }

    /// Gracefully close this websocket.
    pub async fn close(mut self) -> Result<(), BoxError> {
        self.inner.close(None).await.map_err(Into::into)
    }
}

/// A WebSocket message.
#[derive(Eq, PartialEq, Clone)]
pub struct Message {
    inner: protocol::Message,
}

impl Message {
    /// Construct a new Text `Message`.
    pub fn text<S>(s: S) -> Message
    where
        S: Into<String>,
    {
        Message {
            inner: protocol::Message::text(s),
        }
    }

    /// Construct a new Binary `Message`.
    pub fn binary<V>(v: V) -> Message
    where
        V: Into<Vec<u8>>,
    {
        Message {
            inner: protocol::Message::binary(v),
        }
    }

    /// Construct a new Ping `Message`.
    pub fn ping<V: Into<Vec<u8>>>(v: V) -> Message {
        Message {
            inner: protocol::Message::Ping(v.into()),
        }
    }

    /// Construct a new Pong `Message`.
    ///
    /// Note that one rarely needs to manually construct a Pong message because
    /// the underlying tungstenite socket automatically responds to the Ping
    /// messages it receives. Manual construction might still be useful in some
    /// cases like in tests or to send unidirectional heartbeats.
    pub fn pong<V: Into<Vec<u8>>>(v: V) -> Message {
        Message {
            inner: protocol::Message::Pong(v.into()),
        }
    }

    /// Construct the default Close `Message`.
    pub fn close() -> Message {
        Message {
            inner: protocol::Message::Close(None),
        }
    }

    /// Construct a Close `Message` with a code and reason.
    pub fn close_with<C, R>(code: C, reason: R) -> Message
    where
        C: Into<u16>,
        R: Into<Cow<'static, str>>,
    {
        Message {
            inner: protocol::Message::Close(Some(protocol::frame::CloseFrame {
                code: protocol::frame::coding::CloseCode::from(code.into()),
                reason: reason.into(),
            })),
        }
    }

    /// Returns true if this message is a Text message.
    pub fn is_text(&self) -> bool {
        self.inner.is_text()
    }

    /// Returns true if this message is a Binary message.
    pub fn is_binary(&self) -> bool {
        self.inner.is_binary()
    }

    /// Returns true if this message a is a Close message.
    pub fn is_close(&self) -> bool {
        self.inner.is_close()
    }

    /// Returns true if this message is a Ping message.
    pub fn is_ping(&self) -> bool {
        self.inner.is_ping()
    }

    /// Returns true if this message is a Pong message.
    pub fn is_pong(&self) -> bool {
        self.inner.is_pong()
    }

    /// Try to get the close frame (close code and reason)
    pub fn close_frame(&self) -> Option<(u16, &str)> {
        if let protocol::Message::Close(Some(close_frame)) = &self.inner {
            Some((close_frame.code.into(), close_frame.reason.as_ref()))
        } else {
            None
        }
    }

    /// Try to get a reference to the string text, if this is a Text message.
    pub fn to_str(&self) -> Option<&str> {
        if let protocol::Message::Text(s) = &self.inner {
            Some(s)
        } else {
            None
        }
    }

    /// Return the bytes of this message, if the message can contain data.
    pub fn as_bytes(&self) -> &[u8] {
        match self.inner {
            protocol::Message::Text(ref s) => s.as_bytes(),
            protocol::Message::Binary(ref v) => v,
            protocol::Message::Ping(ref v) => v,
            protocol::Message::Pong(ref v) => v,
            protocol::Message::Close(_) => &[],
        }
    }

    /// Destructure this message into binary data.
    pub fn into_bytes(self) -> Vec<u8> {
        self.inner.into_data()
    }
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl From<Message> for Vec<u8> {
    fn from(msg: Message) -> Self {
        msg.into_bytes()
    }
}
