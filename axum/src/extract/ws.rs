//! Handle WebSocket connections.
//!
//! # Example
//!
//! ```
//! use axum::{
//!     extract::ws::{WebSocketUpgrade, WebSocket},
//!     routing::get,
//!     response::IntoResponse,
//!     Router,
//! };
//!
//! let app = Router::new().route("/ws", get(handler));
//!
//! async fn handler(ws: WebSocketUpgrade) -> impl IntoResponse {
//!     ws.on_upgrade(handle_socket)
//! }
//!
//! async fn handle_socket(mut socket: WebSocket) {
//!     while let Some(msg) = socket.recv().await {
//!         let msg = if let Ok(msg) = msg {
//!             msg
//!         } else {
//!             // client disconnected
//!             return;
//!         };
//!
//!         if socket.send(msg).await.is_err() {
//!             // client disconnected
//!             return;
//!         }
//!     }
//! }
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! # Read and write concurrently
//!
//! If you need to read and write concurrently from a [`WebSocket`] you can use
//! [`StreamExt::split`]:
//!
//! ```
//! use axum::{Error, extract::ws::{WebSocket, Message}};
//! use futures::{sink::SinkExt, stream::{StreamExt, SplitSink, SplitStream}};
//!
//! async fn handle_socket(mut socket: WebSocket) {
//!     let (mut sender, mut receiver) = socket.split();
//!
//!     tokio::spawn(write(sender));
//!     tokio::spawn(read(receiver));
//! }
//!
//! async fn read(receiver: SplitStream<WebSocket>) {
//!     // ...
//! }
//!
//! async fn write(sender: SplitSink<WebSocket, Message>) {
//!     // ...
//! }
//! ```
//!
//! [`StreamExt::split`]: https://docs.rs/futures/0.3.17/futures/stream/trait.StreamExt.html#method.split

use self::rejection::*;
use super::{rejection::*, FromRequest, RequestParts};
use crate::{
    body::{self, BoxBody},
    response::IntoResponse,
    Error,
};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{
    sink::{Sink, SinkExt},
    stream::{Stream, StreamExt},
};
use http::{
    header::{self, HeaderName, HeaderValue},
    Method, Response, StatusCode,
};
use hyper::upgrade::{OnUpgrade, Upgraded};
use sha1::{Digest, Sha1};
use std::{
    borrow::Cow,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio_tungstenite::{
    tungstenite::{
        self as ts,
        protocol::{self, WebSocketConfig},
    },
    WebSocketStream,
};

/// Extractor for establishing WebSocket connections.
///
/// Note: This extractor requires the request method to be `GET` so it should
/// always be used with [`get`](crate::routing::get). Requests with other methods will be
/// rejected.
///
/// See the [module docs](self) for an example.
#[derive(Debug)]
pub struct WebSocketUpgrade {
    config: WebSocketConfig,
    protocols: Option<Box<[Cow<'static, str>]>>,
    sec_websocket_key: HeaderValue,
    on_upgrade: OnUpgrade,
    sec_websocket_protocol: Option<HeaderValue>,
}

impl WebSocketUpgrade {
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

    /// Set the known protocols.
    ///
    /// If the protocol name specified by `Sec-WebSocket-Protocol` header
    /// to match any of them, the upgrade response will include `Sec-WebSocket-Protocol` header and
    /// return the protocol name.
    ///
    /// # Examples
    ///
    /// ```
    /// use axum::{
    ///     extract::ws::{WebSocketUpgrade, WebSocket},
    ///     routing::get,
    ///     response::IntoResponse,
    ///     Router,
    /// };
    ///
    /// let app = Router::new().route("/ws", get(handler));
    ///
    /// async fn handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ///     ws.protocols(["graphql-ws", "graphql-transport-ws"])
    ///         .on_upgrade(|socket| async {
    ///             // ...
    ///         })
    /// }
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn protocols<I>(mut self, protocols: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Cow<'static, str>>,
    {
        self.protocols = Some(
            protocols
                .into_iter()
                .map(Into::into)
                .collect::<Vec<_>>()
                .into(),
        );
        self
    }

    /// Finalize upgrading the connection and call the provided callback with
    /// the stream.
    ///
    /// When using `WebSocketUpgrade`, the response produced by this method
    /// should be returned from the handler. See the [module docs](self) for an
    /// example.
    pub fn on_upgrade<F, Fut>(self, callback: F) -> impl IntoResponse
    where
        F: FnOnce(WebSocket) -> Fut + Send + 'static,
        Fut: Future + Send + 'static,
    {
        WebSocketUpgradeResponse {
            extractor: self,
            callback,
        }
    }
}

#[async_trait]
impl<B> FromRequest<B> for WebSocketUpgrade
where
    B: Send,
{
    type Rejection = WebSocketUpgradeRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        if req.method() != Method::GET {
            return Err(MethodNotGet.into());
        }

        if !header_contains(req, header::CONNECTION, "upgrade")? {
            return Err(InvalidConnectionHeader.into());
        }

        if !header_eq(req, header::UPGRADE, "websocket")? {
            return Err(InvalidUpgradeHeader.into());
        }

        if !header_eq(req, header::SEC_WEBSOCKET_VERSION, "13")? {
            return Err(InvalidWebSocketVersionHeader.into());
        }

        let sec_websocket_key = if let Some(key) = req
            .headers_mut()
            .ok_or_else(HeadersAlreadyExtracted::default)?
            .remove(header::SEC_WEBSOCKET_KEY)
        {
            key
        } else {
            return Err(WebSocketKeyHeaderMissing.into());
        };

        let on_upgrade = req
            .extensions_mut()
            .ok_or_else(ExtensionsAlreadyExtracted::default)?
            .remove::<OnUpgrade>()
            .unwrap();

        let sec_websocket_protocol = req
            .headers()
            .ok_or_else(HeadersAlreadyExtracted::default)?
            .get(header::SEC_WEBSOCKET_PROTOCOL)
            .cloned();

        Ok(Self {
            config: Default::default(),
            protocols: None,
            sec_websocket_key,
            on_upgrade,
            sec_websocket_protocol,
        })
    }
}

fn header_eq<B>(
    req: &RequestParts<B>,
    key: HeaderName,
    value: &'static str,
) -> Result<bool, HeadersAlreadyExtracted> {
    if let Some(header) = req
        .headers()
        .ok_or_else(HeadersAlreadyExtracted::default)?
        .get(&key)
    {
        Ok(header.as_bytes().eq_ignore_ascii_case(value.as_bytes()))
    } else {
        Ok(false)
    }
}

fn header_contains<B>(
    req: &RequestParts<B>,
    key: HeaderName,
    value: &'static str,
) -> Result<bool, HeadersAlreadyExtracted> {
    let header = if let Some(header) = req
        .headers()
        .ok_or_else(HeadersAlreadyExtracted::default)?
        .get(&key)
    {
        header
    } else {
        return Ok(false);
    };

    if let Ok(header) = std::str::from_utf8(header.as_bytes()) {
        Ok(header.to_ascii_lowercase().contains(value))
    } else {
        Ok(false)
    }
}

struct WebSocketUpgradeResponse<F> {
    extractor: WebSocketUpgrade,
    callback: F,
}

impl<F, Fut> IntoResponse for WebSocketUpgradeResponse<F>
where
    F: FnOnce(WebSocket) -> Fut + Send + 'static,
    Fut: Future + Send + 'static,
{
    fn into_response(self) -> Response<BoxBody> {
        // check requested protocols
        let protocol = self
            .extractor
            .sec_websocket_protocol
            .as_ref()
            .and_then(|req_protocols| {
                let req_protocols = req_protocols.to_str().ok()?;
                let protocols = self.extractor.protocols.as_ref()?;
                req_protocols
                    .split(',')
                    .map(|req_p| req_p.trim())
                    .find(|req_p| protocols.iter().any(|p| p == req_p))
            });

        let protocol = match protocol {
            Some(protocol) => {
                if let Ok(protocol) = HeaderValue::from_str(protocol) {
                    Some(protocol)
                } else {
                    return (
                        StatusCode::BAD_REQUEST,
                        "`Sec-WebSocket-Protocol` header is invalid",
                    )
                        .into_response();
                }
            }
            None => None,
        };

        let callback = self.callback;
        let on_upgrade = self.extractor.on_upgrade;
        let config = self.extractor.config;

        tokio::spawn(async move {
            let upgraded = on_upgrade.await.expect("connection upgrade failed");
            let socket =
                WebSocketStream::from_raw_socket(upgraded, protocol::Role::Server, Some(config))
                    .await;
            let socket = WebSocket { inner: socket };
            callback(socket).await;
        });

        let mut builder = Response::builder()
            .status(StatusCode::SWITCHING_PROTOCOLS)
            .header(
                header::CONNECTION,
                HeaderValue::from_str("upgrade").unwrap(),
            )
            .header(header::UPGRADE, HeaderValue::from_str("websocket").unwrap())
            .header(
                header::SEC_WEBSOCKET_ACCEPT,
                sign(self.extractor.sec_websocket_key.as_bytes()),
            );

        if let Some(protocol) = protocol {
            builder = builder.header(header::SEC_WEBSOCKET_PROTOCOL, protocol);
        }

        builder.body(body::boxed(body::Empty::new())).unwrap()
    }
}

/// A stream of WebSocket messages.
#[derive(Debug)]
pub struct WebSocket {
    inner: WebSocketStream<Upgraded>,
}

impl WebSocket {
    /// Receive another message.
    ///
    /// Returns `None` if the stream stream has closed.
    pub async fn recv(&mut self) -> Option<Result<Message, Error>> {
        self.next().await
    }

    /// Send a message.
    pub async fn send(&mut self, msg: Message) -> Result<(), Error> {
        self.inner
            .send(msg.into_tungstenite())
            .await
            .map_err(Error::new)
    }

    /// Gracefully close this WebSocket.
    pub async fn close(mut self) -> Result<(), Error> {
        self.inner.close(None).await.map_err(Error::new)
    }
}

impl Stream for WebSocket {
    type Item = Result<Message, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.poll_next_unpin(cx).map(|option_msg| {
            option_msg.map(|result_msg| {
                result_msg
                    .map_err(Error::new)
                    .map(Message::from_tungstenite)
            })
        })
    }
}

impl Sink<Message> for WebSocket {
    type Error = Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner).poll_ready(cx).map_err(Error::new)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        Pin::new(&mut self.inner)
            .start_send(item.into_tungstenite())
            .map_err(Error::new)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx).map_err(Error::new)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner).poll_close(cx).map_err(Error::new)
    }
}

/// Status code used to indicate why an endpoint is closing the WebSocket connection.
pub type CloseCode = u16;

/// A struct representing the close command.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CloseFrame<'t> {
    /// The reason as a code.
    pub code: CloseCode,
    /// The reason as text string.
    pub reason: Cow<'t, str>,
}

/// A WebSocket message.
//
// This code comes from https://github.com/snapview/tungstenite-rs/blob/master/src/protocol/message.rs and is under following license:
// Copyright (c) 2017 Alexey Galakhov
// Copyright (c) 2016 Jason Housley
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Message {
    /// A text WebSocket message
    Text(String),
    /// A binary WebSocket message
    Binary(Vec<u8>),
    /// A ping message with the specified payload
    ///
    /// The payload here must have a length less than 125 bytes
    Ping(Vec<u8>),
    /// A pong message with the specified payload
    ///
    /// The payload here must have a length less than 125 bytes
    Pong(Vec<u8>),
    /// A close message with the optional close frame.
    Close(Option<CloseFrame<'static>>),
}

impl Message {
    fn into_tungstenite(self) -> ts::Message {
        match self {
            Self::Text(text) => ts::Message::Text(text),
            Self::Binary(binary) => ts::Message::Binary(binary),
            Self::Ping(ping) => ts::Message::Ping(ping),
            Self::Pong(pong) => ts::Message::Pong(pong),
            Self::Close(Some(close)) => ts::Message::Close(Some(ts::protocol::CloseFrame {
                code: ts::protocol::frame::coding::CloseCode::from(close.code),
                reason: close.reason,
            })),
            Self::Close(None) => ts::Message::Close(None),
        }
    }

    fn from_tungstenite(message: ts::Message) -> Self {
        match message {
            ts::Message::Text(text) => Self::Text(text),
            ts::Message::Binary(binary) => Self::Binary(binary),
            ts::Message::Ping(ping) => Self::Ping(ping),
            ts::Message::Pong(pong) => Self::Pong(pong),
            ts::Message::Close(Some(close)) => Self::Close(Some(CloseFrame {
                code: close.code.into(),
                reason: close.reason,
            })),
            ts::Message::Close(None) => Self::Close(None),
        }
    }

    /// Consume the WebSocket and return it as binary data.
    pub fn into_data(self) -> Vec<u8> {
        match self {
            Self::Text(string) => string.into_bytes(),
            Self::Binary(data) | Self::Ping(data) | Self::Pong(data) => data,
            Self::Close(None) => Vec::new(),
            Self::Close(Some(frame)) => frame.reason.into_owned().into_bytes(),
        }
    }

    /// Attempt to consume the WebSocket message and convert it to a String.
    pub fn into_text(self) -> Result<String, Error> {
        match self {
            Self::Text(string) => Ok(string),
            Self::Binary(data) | Self::Ping(data) | Self::Pong(data) => Ok(String::from_utf8(data)
                .map_err(|err| err.utf8_error())
                .map_err(Error::new)?),
            Self::Close(None) => Ok(String::new()),
            Self::Close(Some(frame)) => Ok(frame.reason.into_owned()),
        }
    }

    /// Attempt to get a &str from the WebSocket message,
    /// this will try to convert binary data to utf8.
    pub fn to_text(&self) -> Result<&str, Error> {
        match *self {
            Self::Text(ref string) => Ok(string),
            Self::Binary(ref data) | Self::Ping(ref data) | Self::Pong(ref data) => {
                Ok(std::str::from_utf8(data).map_err(Error::new)?)
            }
            Self::Close(None) => Ok(""),
            Self::Close(Some(ref frame)) => Ok(&frame.reason),
        }
    }
}

impl From<Message> for Vec<u8> {
    fn from(msg: Message) -> Self {
        msg.into_data()
    }
}

fn sign(key: &[u8]) -> HeaderValue {
    let mut sha1 = Sha1::default();
    sha1.update(key);
    sha1.update(&b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11"[..]);
    let b64 = Bytes::from(base64::encode(&sha1.finalize()));
    HeaderValue::from_maybe_shared(b64).expect("base64 is a valid value")
}

pub mod rejection {
    //! WebSocket specific rejections.

    use crate::extract::rejection::*;

    define_rejection! {
        #[status = METHOD_NOT_ALLOWED]
        #[body = "Request method must be `GET`"]
        /// Rejection type for [`WebSocketUpgrade`](super::WebSocketUpgrade).
        pub struct MethodNotGet;
    }

    define_rejection! {
        #[status = BAD_REQUEST]
        #[body = "Connection header did not include 'upgrade'"]
        /// Rejection type for [`WebSocketUpgrade`](super::WebSocketUpgrade).
        pub struct InvalidConnectionHeader;
    }

    define_rejection! {
        #[status = BAD_REQUEST]
        #[body = "`Upgrade` header did not include 'websocket'"]
        /// Rejection type for [`WebSocketUpgrade`](super::WebSocketUpgrade).
        pub struct InvalidUpgradeHeader;
    }

    define_rejection! {
        #[status = BAD_REQUEST]
        #[body = "`Sec-WebSocket-Version` header did not include '13'"]
        /// Rejection type for [`WebSocketUpgrade`](super::WebSocketUpgrade).
        pub struct InvalidWebSocketVersionHeader;
    }

    define_rejection! {
        #[status = BAD_REQUEST]
        #[body = "`Sec-WebSocket-Key` header missing"]
        /// Rejection type for [`WebSocketUpgrade`](super::WebSocketUpgrade).
        pub struct WebSocketKeyHeaderMissing;
    }

    composite_rejection! {
        /// Rejection used for [`WebSocketUpgrade`](super::WebSocketUpgrade).
        ///
        /// Contains one variant for each way the [`WebSocketUpgrade`](super::WebSocketUpgrade)
        /// extractor can fail.
        pub enum WebSocketUpgradeRejection {
            MethodNotGet,
            InvalidConnectionHeader,
            InvalidUpgradeHeader,
            InvalidWebSocketVersionHeader,
            WebSocketKeyHeaderMissing,
            HeadersAlreadyExtracted,
            ExtensionsAlreadyExtracted,
        }
    }
}
