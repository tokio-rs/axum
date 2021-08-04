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
//!
//! Websocket handlers can also use extractors, however the first function
//! argument must be of type [`WebSocket`]:
//!
//! ```
//! use axum::{prelude::*, extract::{RequestParts, FromRequest}, ws::{ws, WebSocket}};
//! use http::{HeaderMap, StatusCode};
//!
//! /// An extractor that authorizes requests.
//! struct RequireAuth;
//!
//! #[async_trait::async_trait]
//! impl<B> FromRequest<B> for RequireAuth
//! where
//!     B: Send,
//! {
//!     type Rejection = StatusCode;
//!
//!     async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
//!         # unimplemented!()
//!         // Put your auth logic here...
//!     }
//! }
//!
//! let app = route("/ws", ws(handle_socket));
//!
//! async fn handle_socket(
//!     mut socket: WebSocket,
//!     // Run `RequireAuth` for each request before upgrading.
//!     _auth: RequireAuth,
//! ) {
//!     // ...
//! }
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```

use crate::body::{box_body, BoxBody};
use crate::extract::{FromRequest, RequestParts};
use crate::response::IntoResponse;
use async_trait::async_trait;
use bytes::Bytes;
use future::ResponseFuture;
use futures_util::{
    sink::{Sink, SinkExt},
    stream::{Stream, StreamExt},
};
use http::{
    header::{self, HeaderName},
    HeaderValue, Request, Response, StatusCode,
};
use http_body::Full;
use hyper::upgrade::{OnUpgrade, Upgraded};
use sha1::{Digest, Sha1};
use std::pin::Pin;
use std::sync::Arc;
use std::{
    borrow::Cow, convert::Infallible, fmt, future::Future, marker::PhantomData, task::Context,
    task::Poll,
};
use tokio_tungstenite::{
    tungstenite,
    tungstenite::protocol::{self, WebSocketConfig},
    WebSocketStream,
};
use tower::{BoxError, Service};

mod coding;
pub use coding::CloseCode;
pub mod future;

/// Create a new [`WebSocketUpgrade`] service that will call the closure with
/// each connection.
///
/// See the [module docs](crate::ws) for more details.
pub fn ws<F, B, T>(callback: F) -> WebSocketUpgrade<F, B, T>
where
    F: WebSocketHandler<B, T>,
{
    WebSocketUpgrade {
        callback,
        config: WebSocketConfig::default(),
        protocols: Vec::new().into(),
        _request_body: PhantomData,
    }
}

/// Trait for async functions that can be used to handle websocket requests.
///
/// You shouldn't need to depend on this trait directly. It is automatically
/// implemented to closures of the right types.
///
/// See the [module docs](crate::ws) for more details.
#[async_trait]
pub trait WebSocketHandler<B, In>: Sized {
    // This seals the trait. We cannot use the regular "sealed super trait"
    // approach due to coherence.
    #[doc(hidden)]
    type Sealed: crate::handler::sealed::HiddentTrait;

    /// Call the handler with the given websocket stream and input parsed by
    /// extractors.
    async fn call(self, stream: WebSocket, input: In);
}

#[async_trait]
impl<F, Fut, B> WebSocketHandler<B, ()> for F
where
    F: FnOnce(WebSocket) -> Fut + Send,
    Fut: Future<Output = ()> + Send,
    B: Send,
{
    type Sealed = crate::handler::sealed::Hidden;

    async fn call(self, stream: WebSocket, _: ()) {
        self(stream).await
    }
}

macro_rules! impl_ws_handler {
    () => {
    };

    ( $head:ident, $($tail:ident),* $(,)? ) => {
        #[async_trait]
        #[allow(non_snake_case)]
        impl<F, Fut, B, $head, $($tail,)*> WebSocketHandler<B, ($head, $($tail,)*)> for F
        where
            B: Send,
            $head: FromRequest<B> + Send + 'static,
            $( $tail: FromRequest<B> + Send + 'static, )*
            F: FnOnce(WebSocket, $head, $($tail,)*) -> Fut + Send,
            Fut: Future<Output = ()> + Send,
        {
            type Sealed = crate::handler::sealed::Hidden;

            async fn call(self, stream: WebSocket, ($head, $($tail,)*): ($head, $($tail,)*)) {
                self(stream, $head, $($tail,)*).await
            }
        }

        impl_ws_handler!($($tail,)*);
    };
}

impl_ws_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);

/// [`Service`] that upgrades connections to websockets and spawns a task to
/// handle the stream.
///
/// Created with [`ws`].
///
/// See the [module docs](crate::ws) for more details.
pub struct WebSocketUpgrade<F, B, T> {
    callback: F,
    config: WebSocketConfig,
    protocols: Arc<[Cow<'static, str>]>,
    _request_body: PhantomData<fn() -> (B, T)>,
}

impl<F, B, T> Clone for WebSocketUpgrade<F, B, T>
where
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            config: self.config,
            protocols: self.protocols.clone(),
            _request_body: PhantomData,
        }
    }
}

impl<F, B, T> fmt::Debug for WebSocketUpgrade<F, B, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebSocketUpgrade")
            .field("callback", &format_args!("{}", std::any::type_name::<F>()))
            .field("config", &self.config)
            .finish()
    }
}

impl<F, B, T> WebSocketUpgrade<F, B, T> {
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
    /// to match any of them, the upgrade response will include `Sec-WebSocket-Protocol` header and return the protocol name.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use axum::prelude::*;
    /// # use axum::ws::{ws, WebSocket};
    /// # use std::net::SocketAddr;
    /// #
    /// # async fn handle_socket(socket: WebSocket) {
    /// #     todo!()
    /// # }
    /// #
    /// # #[tokio::main]
    /// # async fn main() {
    /// let app = route("/ws", ws(handle_socket).protocols(["graphql-ws", "graphql-transport-ws"]));
    /// #
    /// #   let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    /// #   hyper::Server::bind(&addr)
    /// #       .serve(app.into_make_service())
    /// #       .await
    /// #       .unwrap();
    /// # }
    ///
    /// ```
    pub fn protocols<I>(mut self, protocols: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Cow<'static, str>>,
    {
        self.protocols = protocols
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>()
            .into();
        self
    }
}

impl<ReqBody, F, T> Service<Request<ReqBody>> for WebSocketUpgrade<F, ReqBody, T>
where
    F: WebSocketHandler<ReqBody, T> + Clone + Send + 'static,
    T: FromRequest<ReqBody> + Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = ResponseFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        let this = self.clone();
        let protocols = self.protocols.clone();

        ResponseFuture {
            future: Box::pin(async move {
                if req.method() != http::Method::GET {
                    return response(StatusCode::NOT_FOUND, "Request method must be `GET`");
                }

                if !header_contains(&req, header::CONNECTION, "upgrade") {
                    return response(
                        StatusCode::BAD_REQUEST,
                        "Connection header did not include 'upgrade'",
                    );
                }

                if !header_eq(&req, header::UPGRADE, "websocket") {
                    return response(
                        StatusCode::BAD_REQUEST,
                        "`Upgrade` header did not include 'websocket'",
                    );
                }

                if !header_eq(&req, header::SEC_WEBSOCKET_VERSION, "13") {
                    return response(
                        StatusCode::BAD_REQUEST,
                        "`Sec-Websocket-Version` header did not include '13'",
                    );
                }

                // check requested protocols
                let protocol =
                    req.headers()
                        .get(&header::SEC_WEBSOCKET_PROTOCOL)
                        .and_then(|req_protocols| {
                            let req_protocols = req_protocols.to_str().ok()?;
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
                            return response(
                                StatusCode::BAD_REQUEST,
                                "`Sec-Websocket-Protocol` header is invalid",
                            );
                        }
                    }
                    None => None,
                };

                let key = if let Some(key) = req.headers_mut().remove(header::SEC_WEBSOCKET_KEY) {
                    key
                } else {
                    return response(
                        StatusCode::BAD_REQUEST,
                        "`Sec-Websocket-Key` header missing",
                    );
                };

                let on_upgrade = req.extensions_mut().remove::<OnUpgrade>().unwrap();

                let config = this.config;
                let callback = this.callback.clone();

                let mut req = RequestParts::new(req);
                let input = match T::from_request(&mut req).await {
                    Ok(input) => input,
                    Err(rejection) => {
                        let res = rejection.into_response().map(box_body);
                        return Ok(res);
                    }
                };

                tokio::spawn(async move {
                    let upgraded = on_upgrade.await.unwrap();
                    let socket = WebSocketStream::from_raw_socket(
                        upgraded,
                        protocol::Role::Server,
                        Some(config),
                    )
                    .await;
                    let socket = WebSocket { inner: socket };
                    callback.call(socket, input).await;
                });

                let mut builder = Response::builder()
                    .status(StatusCode::SWITCHING_PROTOCOLS)
                    .header(
                        http::header::CONNECTION,
                        HeaderValue::from_str("upgrade").unwrap(),
                    )
                    .header(
                        http::header::UPGRADE,
                        HeaderValue::from_str("websocket").unwrap(),
                    )
                    .header(http::header::SEC_WEBSOCKET_ACCEPT, sign(key.as_bytes()));
                if let Some(protocol) = protocol {
                    builder = builder.header(http::header::SEC_WEBSOCKET_PROTOCOL, protocol);
                }
                let res = builder.body(box_body(Full::new(Bytes::new()))).unwrap();

                Ok(res)
            }),
        }
    }
}

fn response<E>(status: StatusCode, body: &'static str) -> Result<Response<BoxBody>, E> {
    let res = Response::builder()
        .status(status)
        .body(box_body(Full::from(body)))
        .unwrap();
    Ok(res)
}

fn header_eq<B>(req: &Request<B>, key: HeaderName, value: &'static str) -> bool {
    if let Some(header) = req.headers().get(&key) {
        header.as_bytes().eq_ignore_ascii_case(value.as_bytes())
    } else {
        false
    }
}

fn header_contains<B>(req: &Request<B>, key: HeaderName, value: &'static str) -> bool {
    let header = if let Some(header) = req.headers().get(&key) {
        header
    } else {
        return false;
    };

    if let Ok(header) = std::str::from_utf8(header.as_bytes()) {
        header.to_ascii_lowercase().contains(value)
    } else {
        false
    }
}

fn sign(key: &[u8]) -> HeaderValue {
    let mut sha1 = Sha1::default();
    sha1.update(key);
    sha1.update(&b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11"[..]);
    let b64 = Bytes::from(base64::encode(&sha1.finalize()));
    HeaderValue::from_maybe_shared(b64).expect("base64 is a valid value")
}

/// A stream of websocket messages.
#[derive(Debug)]
pub struct WebSocket {
    inner: WebSocketStream<Upgraded>,
}

impl WebSocket {
    /// Receive another message.
    ///
    /// Returns `None` if the stream stream has closed.
    pub async fn recv(&mut self) -> Option<Result<Message, BoxError>> {
        self.next().await
    }

    /// Send a message.
    pub async fn send(&mut self, msg: Message) -> Result<(), BoxError> {
        self.inner
            .send(msg.into_tungstenite())
            .await
            .map_err(Into::into)
    }

    /// Gracefully close this websocket.
    pub async fn close(mut self) -> Result<(), BoxError> {
        self.inner.close(None).await.map_err(Into::into)
    }
}

impl Stream for WebSocket {
    type Item = Result<Message, BoxError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.poll_next_unpin(cx).map(|option_msg| {
            option_msg.map(|result_msg| {
                result_msg
                    .map_err(Into::into)
                    .map(Message::from_tungstenite)
            })
        })
    }
}

impl Sink<Message> for WebSocket {
    type Error = BoxError;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner).poll_ready(cx).map_err(Into::into)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        Pin::new(&mut self.inner)
            .start_send(item.into_tungstenite())
            .map_err(Into::into)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx).map_err(Into::into)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner).poll_close(cx).map_err(Into::into)
    }
}

/// A struct representing the close command.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CloseFrame<'t> {
    /// The reason as a code.
    pub code: coding::CloseCode,
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
    fn into_tungstenite(self) -> tungstenite::Message {
        // TODO: maybe some shorter way to do that?
        match self {
            Self::Text(text) => tungstenite::Message::Text(text),
            Self::Binary(binary) => tungstenite::Message::Binary(binary),
            Self::Ping(ping) => tungstenite::Message::Ping(ping),
            Self::Pong(pong) => tungstenite::Message::Pong(pong),
            Self::Close(Some(close)) => {
                let code: u16 = close.code.into();
                tungstenite::Message::Close(Some(tungstenite::protocol::CloseFrame {
                    code: tungstenite::protocol::frame::coding::CloseCode::from(code),
                    reason: close.reason,
                }))
            }
            Self::Close(None) => tungstenite::Message::Close(None),
        }
    }

    fn from_tungstenite(message: tungstenite::Message) -> Self {
        // TODO: maybe some shorter way to do that?
        match message {
            tungstenite::Message::Text(text) => Self::Text(text),
            tungstenite::Message::Binary(binary) => Self::Binary(binary),
            tungstenite::Message::Ping(ping) => Self::Ping(ping),
            tungstenite::Message::Pong(pong) => Self::Pong(pong),
            tungstenite::Message::Close(Some(close)) => {
                let code: u16 = close.code.into();
                Self::Close(Some(CloseFrame {
                    code: CloseCode::from(code),
                    reason: close.reason,
                }))
            }
            tungstenite::Message::Close(None) => Self::Close(None),
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
    pub fn into_text(self) -> Result<String, BoxError> {
        match self {
            Self::Text(string) => Ok(string),
            Self::Binary(data) | Self::Ping(data) | Self::Pong(data) => {
                Ok(String::from_utf8(data).map_err(|err| err.utf8_error())?)
            }
            Self::Close(None) => Ok(String::new()),
            Self::Close(Some(frame)) => Ok(frame.reason.into_owned()),
        }
    }

    /// Attempt to get a &str from the WebSocket message,
    /// this will try to convert binary data to utf8.
    pub fn to_text(&self) -> Result<&str, BoxError> {
        match *self {
            Self::Text(ref string) => Ok(string),
            Self::Binary(ref data) | Self::Ping(ref data) | Self::Pong(ref data) => {
                Ok(std::str::from_utf8(data)?)
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
