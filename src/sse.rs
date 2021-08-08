//! Server-Sent Events (SSE)
//!
//! # Example
//!
//! ```
//! use axum::{prelude::*, sse::{sse, Event, KeepAlive}};
//! use tokio_stream::StreamExt as _;
//! use futures::stream::{self, Stream};
//! use std::{
//!     time::Duration,
//!     convert::Infallible,
//! };
//!
//! let app = route("/sse", sse(make_stream).keep_alive(KeepAlive::default()));
//!
//! async fn make_stream(
//! ) -> Result<impl Stream<Item = Result<Event, Infallible>>, Infallible> {
//!     // A `Stream` that repeats an event every second
//!     let stream = stream::repeat_with(|| Event::default().data("hi!"))
//!         .map(Ok)
//!         .throttle(Duration::from_secs(1));
//!
//!     Ok(stream)
//! }
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! SSE handlers can also use extractors:
//!
//! ```
//! use axum::{prelude::*, sse::{sse, Event}, extract::{RequestParts, FromRequest}};
//! use tokio_stream::StreamExt as _;
//! use futures::stream::{self, Stream};
//! use std::{
//!     time::Duration,
//!     convert::Infallible,
//! };
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
//! let app = route("/sse", sse(make_stream));
//!
//! async fn make_stream(
//!     // Run `RequireAuth` for each request before initiating the stream.
//!     _auth: RequireAuth,
//! ) -> Result<impl Stream<Item = Result<Event, Infallible>>, Infallible> {
//!     // ...
//!     # Ok(futures::stream::pending())
//! }
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```

use crate::{
    body::{box_body, BoxBody},
    extract::{FromRequest, RequestParts},
    response::IntoResponse,
    Error,
};
use async_trait::async_trait;
use futures_util::{
    future::{TryFuture, TryFutureExt},
    stream::{Stream, StreamExt, TryStream, TryStreamExt},
};
use http::{Request, Response};
use hyper::Body;
use pin_project_lite::pin_project;
use serde::Serialize;
use std::{
    borrow::Cow,
    convert::Infallible,
    fmt::{self, Write},
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::time::Sleep;
use tower::{BoxError, Service};

/// Create a new [`Sse`] service that will call the closure to produce a stream
/// of [`Event`]s.
///
/// See the [module docs](crate::sse) for more details.
pub fn sse<H, B, T>(handler: H) -> Sse<H, B, T>
where
    H: SseHandler<B, T>,
{
    Sse {
        handler,
        keep_alive: None,
        _request_body: PhantomData,
    }
}

/// Trait for async functions that can be used to handle Server-sent event
/// requests.
///
/// You shouldn't need to depend on this trait directly. It is automatically
/// implemented to closures of the right types.
///
/// See the [module docs](crate::sse) for more details.
#[async_trait]
pub trait SseHandler<B, In>: Sized {
    /// The stream of events produced by the handler.
    type Stream: TryStream<Ok = Event> + Send + 'static;

    /// The error handler might fail with.
    type Error: IntoResponse;

    // This seals the trait. We cannot use the regular "sealed super trait"
    // approach due to coherence.
    #[doc(hidden)]
    type Sealed: crate::handler::sealed::HiddentTrait;

    /// Call the handler with the given input parsed by extractors and produce
    /// the stream of events.
    async fn call(self, input: In) -> Result<Self::Stream, Self::Error>;
}

#[async_trait]
impl<F, Fut, S, B> SseHandler<B, ()> for F
where
    F: FnOnce() -> Fut + Send,
    Fut: TryFuture<Ok = S> + Send,
    Fut::Error: IntoResponse,
    S: TryStream<Ok = Event> + Send + 'static,
{
    type Stream = S;
    type Error = Fut::Error;
    type Sealed = crate::handler::sealed::Hidden;

    async fn call(self, _: ()) -> Result<Self::Stream, Self::Error> {
        self().into_future().await
    }
}

macro_rules! impl_sse_handler {
    () => {
    };

    ( $head:ident, $($tail:ident),* $(,)? ) => {
        #[async_trait]
        #[allow(non_snake_case)]
        impl<F, Fut, S, B, $head, $($tail,)*> SseHandler<B, ($head, $($tail,)*)> for F
        where
            B: Send,
            F: FnOnce($head, $($tail,)*) -> Fut + Send,
            Fut: TryFuture<Ok = S> + Send,
            Fut::Error: IntoResponse,
            S: TryStream<Ok = Event> + Send + 'static,
            $head: FromRequest<B> + Send + 'static,
            $( $tail: FromRequest<B> + Send + 'static, )*
        {
            type Stream = S;
            type Error = Fut::Error;
            type Sealed = crate::handler::sealed::Hidden;

            async fn call(self, ($head, $($tail,)*): ($head, $($tail,)*)) -> Result<Self::Stream, Self::Error> {
                self($head, $($tail,)*).into_future().await
            }
        }

        impl_sse_handler!($($tail,)*);
    };
}

impl_sse_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);

/// [`Service`] that handlers streams of Server-sent events.
///
/// See the [module docs](crate::sse) for more details.
pub struct Sse<H, B, T> {
    handler: H,
    keep_alive: Option<KeepAlive>,
    _request_body: PhantomData<fn() -> (B, T)>,
}

impl<H, B, T> Sse<H, B, T> {
    /// Configure the interval between keep-alive messages.
    ///
    /// Defaults to no keep-alive messages.
    pub fn keep_alive(mut self, keep_alive: KeepAlive) -> Self {
        self.keep_alive = Some(keep_alive);
        self
    }
}

impl<H, B, T> fmt::Debug for Sse<H, B, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sse")
            .field("handler", &format_args!("{}", std::any::type_name::<H>()))
            .field("keep_alive", &self.keep_alive)
            .finish()
    }
}

impl<H, B, T> Clone for Sse<H, B, T>
where
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            keep_alive: self.keep_alive.clone(),
            _request_body: PhantomData,
        }
    }
}

impl<ReqBody, H, T> Service<Request<ReqBody>> for Sse<H, ReqBody, T>
where
    H: SseHandler<ReqBody, T> + Clone + Send + 'static,
    T: FromRequest<ReqBody> + Send,
    ReqBody: Send + 'static,
    <H::Stream as TryStream>::Error: Into<BoxError>,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = ResponseFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let handler = self.handler.clone();
        let keep_alive = self.keep_alive.clone();

        ResponseFuture {
            future: Box::pin(async move {
                let mut req = RequestParts::new(req);
                let input = match T::from_request(&mut req).await {
                    Ok(input) => input,
                    Err(err) => {
                        return Ok(err.into_response().map(box_body));
                    }
                };

                let stream = match handler.call(input).await {
                    Ok(stream) => stream,
                    Err(err) => {
                        return Ok(err.into_response().map(box_body));
                    }
                };

                let stream = if let Some(keep_alive) = keep_alive {
                    KeepAliveStream {
                        event_stream: stream,
                        comment_text: keep_alive.comment_text,
                        max_interval: keep_alive.max_interval,
                        alive_timer: tokio::time::sleep(keep_alive.max_interval),
                    }
                    .left_stream()
                } else {
                    stream.into_stream().right_stream()
                };

                let stream = stream
                    .map_ok(|event| event.to_string())
                    .map_err(Error::new)
                    .into_stream();

                let body = box_body(Body::wrap_stream(stream));

                let response = Response::builder()
                    .header(http::header::CONTENT_TYPE, "text/event-stream")
                    .header(http::header::CACHE_CONTROL, "no-cache")
                    .body(body)
                    .unwrap();

                Ok(response)
            }),
        }
    }
}

opaque_future! {
    /// Response future for [`Sse`].
    pub type ResponseFuture =
        futures_util::future::BoxFuture<'static, Result<Response<BoxBody>, Infallible>>;
}

/// Server-sent event
#[derive(Default, Debug)]
pub struct Event {
    name: Option<String>,
    id: Option<String>,
    data: Option<DataType>,
    event: Option<String>,
    comment: Option<String>,
    retry: Option<Duration>,
}

// Server-sent event data type
#[derive(Debug)]
enum DataType {
    Text(String),
    Json(String),
}

impl Event {
    /// Set Server-sent event data
    /// data field(s) ("data:<content>")
    pub fn data<T>(mut self, data: T) -> Event
    where
        T: Into<String>,
    {
        self.data = Some(DataType::Text(data.into()));
        self
    }

    /// Set Server-sent event data
    /// data field(s) ("data:<content>")
    pub fn json_data<T>(mut self, data: T) -> Result<Event, serde_json::Error>
    where
        T: Serialize,
    {
        self.data = Some(DataType::Json(serde_json::to_string(&data)?));
        Ok(self)
    }

    /// Set Server-sent event comment
    /// Comment field (":<comment-text>")
    pub fn comment<T>(mut self, comment: T) -> Event
    where
        T: Into<String>,
    {
        self.comment = Some(comment.into());
        self
    }

    /// Set Server-sent event event
    /// Event name field ("event:<event-name>")
    pub fn event<T>(mut self, event: T) -> Event
    where
        T: Into<String>,
    {
        self.event = Some(event.into());
        self
    }

    /// Set Server-sent event retry
    /// Retry timeout field ("retry:<timeout>")
    pub fn retry(mut self, duration: Duration) -> Event {
        self.retry = Some(duration);
        self
    }

    /// Set Server-sent event id
    /// Identifier field ("id:<identifier>")
    pub fn id<T>(mut self, id: T) -> Event
    where
        T: Into<String>,
    {
        self.id = Some(id.into());
        self
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(comment) = &self.comment {
            ":".fmt(f)?;
            comment.fmt(f)?;
            f.write_char('\n')?;
        }

        if let Some(event) = &self.event {
            "event:".fmt(f)?;
            event.fmt(f)?;
            f.write_char('\n')?;
        }

        match &self.data {
            Some(DataType::Text(data)) => {
                for line in data.split('\n') {
                    "data:".fmt(f)?;
                    line.fmt(f)?;
                    f.write_char('\n')?;
                }
            }
            Some(DataType::Json(data)) => {
                "data:".fmt(f)?;
                data.fmt(f)?;
                f.write_char('\n')?;
            }
            None => {}
        }

        if let Some(id) = &self.id {
            "id:".fmt(f)?;
            id.fmt(f)?;
            f.write_char('\n')?;
        }

        if let Some(duration) = &self.retry {
            "retry:".fmt(f)?;

            let secs = duration.as_secs();
            let millis = duration.subsec_millis();

            if secs > 0 {
                // format seconds
                secs.fmt(f)?;

                // pad milliseconds
                if millis < 10 {
                    f.write_str("00")?;
                } else if millis < 100 {
                    f.write_char('0')?;
                }
            }

            // format milliseconds
            millis.fmt(f)?;

            f.write_char('\n')?;
        }

        f.write_char('\n')?;

        Ok(())
    }
}

/// Configure the interval between keep-alive messages, the content
/// of each message, and the associated stream.
#[derive(Debug, Clone)]
pub struct KeepAlive {
    comment_text: Cow<'static, str>,
    max_interval: Duration,
}

impl KeepAlive {
    /// Create a new `KeepAlive`.
    pub fn new() -> Self {
        Self {
            comment_text: Cow::Borrowed(""),
            max_interval: Duration::from_secs(15),
        }
    }

    /// Customize the interval between keep-alive messages.
    ///
    /// Default is 15 seconds.
    pub fn interval(mut self, time: Duration) -> Self {
        self.max_interval = time;
        self
    }

    /// Customize the text of the keep-alive message.
    ///
    /// Default is an empty comment.
    pub fn text<I>(mut self, text: I) -> Self
    where
        I: Into<Cow<'static, str>>,
    {
        self.comment_text = text.into();
        self
    }
}

impl Default for KeepAlive {
    fn default() -> Self {
        Self::new()
    }
}

pin_project! {
    struct KeepAliveStream<S> {
        #[pin]
        event_stream: S,
        comment_text: Cow<'static, str>,
        max_interval: Duration,
        #[pin]
        alive_timer: Sleep,
    }
}

impl<S> Stream for KeepAliveStream<S>
where
    S: TryStream<Ok = Event>,
{
    type Item = Result<Event, S::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        match this.event_stream.try_poll_next(cx) {
            Poll::Pending => match Pin::new(&mut this.alive_timer).poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(_) => {
                    // restart timer
                    this.alive_timer
                        .reset(tokio::time::Instant::now() + *this.max_interval);

                    let comment_str = this.comment_text.clone();
                    let event = Event::default().comment(comment_str);
                    Poll::Ready(Some(Ok(event)))
                }
            },
            Poll::Ready(Some(Ok(event))) => {
                // restart timer
                this.alive_timer
                    .reset(tokio::time::Instant::now() + *this.max_interval);

                Poll::Ready(Some(Ok(event)))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Ready(Some(Err(error))) => Poll::Ready(Some(Err(error))),
        }
    }
}
