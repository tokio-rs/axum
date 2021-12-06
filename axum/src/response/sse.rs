//! Server-Sent Events (SSE) responses.
//!
//! # Example
//!
//! ```
//! use axum::{
//!     Router,
//!     routing::get,
//!     response::sse::{Event, KeepAlive, Sse},
//! };
//! use std::{time::Duration, convert::Infallible};
//! use tokio_stream::StreamExt as _ ;
//! use futures::stream::{self, Stream};
//!
//! let app = Router::new().route("/sse", get(sse_handler));
//!
//! async fn sse_handler() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
//!     // A `Stream` that repeats an event every second
//!     let stream = stream::repeat_with(|| Event::default().data("hi!"))
//!         .map(Ok)
//!         .throttle(Duration::from_secs(1));
//!
//!     Sse::new(stream).keep_alive(KeepAlive::default())
//! }
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```

use crate::{
    body,
    response::{IntoResponse, Response},
    BoxError,
};
use bytes::{BufMut, Bytes, BytesMut};
use futures_util::{
    ready,
    stream::{Stream, TryStream},
};
use http_body::Body as HttpBody;
use pin_project_lite::pin_project;
use std::{
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use sync_wrapper::SyncWrapper;
use tokio::time::Sleep;

/// An SSE response
#[derive(Clone)]
pub struct Sse<S> {
    stream: S,
    keep_alive: Option<KeepAlive>,
}

impl<S> Sse<S> {
    /// Create a new [`Sse`] response that will respond with the given stream of
    /// [`Event`]s.
    ///
    /// See the [module docs](self) for more details.
    pub fn new(stream: S) -> Self
    where
        S: TryStream<Ok = Event> + Send + 'static,
        S::Error: Into<BoxError>,
    {
        Sse {
            stream,
            keep_alive: None,
        }
    }

    /// Configure the interval between keep-alive messages.
    ///
    /// Defaults to no keep-alive messages.
    pub fn keep_alive(mut self, keep_alive: KeepAlive) -> Self {
        self.keep_alive = Some(keep_alive);
        self
    }
}

impl<S> fmt::Debug for Sse<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sse")
            .field("stream", &format_args!("{}", std::any::type_name::<S>()))
            .field("keep_alive", &self.keep_alive)
            .finish()
    }
}

impl<S, E> IntoResponse for Sse<S>
where
    S: Stream<Item = Result<Event, E>> + Send + 'static,
    E: Into<BoxError>,
{
    fn into_response(self) -> Response {
        let body = body::boxed(Body {
            event_stream: SyncWrapper::new(self.stream),
            keep_alive: self.keep_alive.map(KeepAliveStream::new),
        });

        Response::builder()
            .header(http::header::CONTENT_TYPE, mime::TEXT_EVENT_STREAM.as_ref())
            .header(http::header::CACHE_CONTROL, "no-cache")
            .body(body)
            .unwrap()
    }
}

pin_project! {
    struct Body<S> {
        #[pin]
        event_stream: SyncWrapper<S>,
        #[pin]
        keep_alive: Option<KeepAliveStream>,
    }
}

impl<S, E> HttpBody for Body<S>
where
    S: Stream<Item = Result<Event, E>>,
{
    type Data = Bytes;
    type Error = E;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        let this = self.project();

        match this.event_stream.get_pin_mut().poll_next(cx) {
            Poll::Pending => {
                if let Some(keep_alive) = this.keep_alive.as_pin_mut() {
                    keep_alive.poll_event(cx).map(|e| Some(Ok(e)))
                } else {
                    Poll::Pending
                }
            }
            Poll::Ready(Some(Ok(event))) => {
                if let Some(keep_alive) = this.keep_alive.as_pin_mut() {
                    keep_alive.reset();
                }
                Poll::Ready(Some(Ok(event.finalize())))
            }
            Poll::Ready(Some(Err(error))) => Poll::Ready(Some(Err(error))),
            Poll::Ready(None) => Poll::Ready(None),
        }
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }
}

/// A server-sent event.
#[derive(Debug, Default, Clone)]
pub struct Event {
    buffer: BytesMut,
    flags: EventFlags,
}

impl Event {
    /// Set the event's data field(s) ("data:<content>").
    ///
    /// Newlines in `data` will automatically be broken across multiple `data:` fields.
    ///
    /// # Panics
    ///
    /// - Panics if `data` contains any carriage returns, as they cannot be transmitted over SSE.
    /// - Panics if `data` or `json_data` has already been called.
    pub fn data<T>(mut self, data: T) -> Self
    where
        T: AsRef<str>,
    {
        if self.flags.contains(EventFlags::HAS_DATA) {
            panic!("Called `EventBuilder::data` multiple times");
        }

        for line in memchr_split(b'\n', data.as_ref().as_bytes()) {
            self.field("data", line);
        }

        self.flags.insert(EventFlags::HAS_DATA);

        self
    }

    /// Set the event's data field to a serialized JSON value ("data:<content>").
    ///
    /// # Panics
    ///
    /// Panics if `data` or `json_data` has already been called.
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    pub fn json_data<T>(mut self, data: T) -> serde_json::Result<Self>
    where
        T: serde::Serialize,
    {
        if self.flags.contains(EventFlags::HAS_DATA) {
            panic!("Called `EventBuilder::json_data` multiple times");
        }

        self.buffer.extend_from_slice(b"data:");
        serde_json::to_writer((&mut self.buffer).writer(), &data)?;
        self.buffer.put_u8(b'\n');

        self.flags.insert(EventFlags::HAS_DATA);

        Ok(self)
    }

    /// Add a comment field to the event (":<comment-text>").
    ///
    /// Unlike other functions, this function can be called multiple times to add many comments.
    ///
    /// # Panics
    ///
    /// Panics if `comment` contains any newlines or carriage returns, as they are not allowed in
    /// comments.
    pub fn comment<T>(mut self, comment: T) -> Self
    where
        T: AsRef<str>,
    {
        self.field("", comment.as_ref());
        self
    }

    /// Set the event's name field ("event:<event-name>").
    ///
    /// # Panics
    ///
    /// - Panics if `event` contains any newlines or carriage returns.
    /// - Panics if this function has already been called on this event.
    pub fn event<T>(mut self, event: T) -> Self
    where
        T: AsRef<str>,
    {
        if self.flags.contains(EventFlags::HAS_EVENT) {
            panic!("Called `EventBuilder::event` multiple times");
        }
        self.flags.insert(EventFlags::HAS_EVENT);

        self.field("event", event.as_ref());
        self
    }

    /// Set the event's retry timeout field ("retry:<timeout>").
    ///
    /// # Panics
    ///
    /// Panics if this function has already been called on this event.
    pub fn retry(mut self, duration: Duration) -> Self {
        if self.flags.contains(EventFlags::HAS_RETRY) {
            panic!("Called `EventBuilder::retry` multiple times");
        }
        self.flags.insert(EventFlags::HAS_RETRY);

        self.buffer.extend_from_slice(b"retry:");

        let secs = duration.as_secs();
        let millis = duration.subsec_millis();

        if secs > 0 {
            // format seconds
            itoa::fmt(&mut self.buffer, secs).unwrap();

            // pad milliseconds
            if millis < 10 {
                self.buffer.extend_from_slice(b"00");
            } else if millis < 100 {
                self.buffer.extend_from_slice(b"0");
            }
        }

        // format milliseconds
        itoa::fmt(&mut self.buffer, millis).unwrap();

        self.buffer.put_u8(b'\n');

        self
    }

    /// Set the event's identifier field ("id:<identifier>").
    ///
    /// # Panics
    ///
    /// - Panics if `id` contains any newlines, carriage returns or null characters.
    /// - Panics if this function has already been called on this event.
    pub fn id<T>(mut self, id: T) -> Self
    where
        T: AsRef<str>,
    {
        if self.flags.contains(EventFlags::HAS_ID) {
            panic!("Called `EventBuilder::id` multiple times");
        }
        self.flags.insert(EventFlags::HAS_ID);

        let id = id.as_ref().as_bytes();
        assert_eq!(
            memchr::memchr(b'\0', id),
            None,
            "Event ID cannot contain null characters",
        );

        self.field("id", id);
        self
    }

    fn field(&mut self, name: &str, value: impl AsRef<[u8]>) {
        let value = value.as_ref();
        assert_eq!(
            memchr::memchr2(b'\r', b'\n', value),
            None,
            "SSE field value cannot contain newlines or carriage returns",
        );
        self.buffer.extend_from_slice(name.as_bytes());
        self.buffer.put_u8(b':');
        // Prevent values that start with spaces having that space stripped
        if value.starts_with(b" ") {
            self.buffer.put_u8(b' ');
        }
        self.buffer.extend_from_slice(value);
        self.buffer.put_u8(b'\n');
    }

    fn finalize(mut self) -> Bytes {
        self.buffer.put_u8(b'\n');
        self.buffer.freeze()
    }
}

bitflags::bitflags! {
    #[derive(Default)]
    struct EventFlags: u8 {
        const HAS_DATA  = 0b0001;
        const HAS_EVENT = 0b0010;
        const HAS_RETRY = 0b0100;
        const HAS_ID    = 0b1000;
    }
}

/// Configure the interval between keep-alive messages, the content
/// of each message, and the associated stream.
#[derive(Debug, Clone)]
pub struct KeepAlive {
    event: Bytes,
    max_interval: Duration,
}

impl KeepAlive {
    /// Create a new `KeepAlive`.
    pub fn new() -> Self {
        Self {
            event: Bytes::from_static(b":\n\n"),
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
    ///
    /// # Panics
    ///
    /// Panics if `text` contains any newline or carriage returns, as they are not allowed in SSE
    /// comments.
    pub fn text<I>(mut self, text: I) -> Self
    where
        I: AsRef<str>,
    {
        self.event = Event::default().comment(text).finalize();
        self
    }
}

impl Default for KeepAlive {
    fn default() -> Self {
        Self::new()
    }
}

pin_project! {
    #[derive(Debug)]
    struct KeepAliveStream {
        keep_alive: KeepAlive,
        #[pin]
        alive_timer: Sleep,
    }
}

impl KeepAliveStream {
    fn new(keep_alive: KeepAlive) -> Self {
        Self {
            alive_timer: tokio::time::sleep(keep_alive.max_interval),
            keep_alive,
        }
    }

    fn reset(self: Pin<&mut Self>) {
        let this = self.project();
        this.alive_timer
            .reset(tokio::time::Instant::now() + this.keep_alive.max_interval);
    }

    fn poll_event(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Bytes> {
        let this = self.as_mut().project();

        ready!(this.alive_timer.poll(cx));

        let event = this.keep_alive.event.clone();

        self.reset();

        Poll::Ready(event)
    }
}

fn memchr_split(needle: u8, haystack: &[u8]) -> MemchrSplit<'_> {
    MemchrSplit {
        needle,
        haystack: Some(haystack),
    }
}

struct MemchrSplit<'a> {
    needle: u8,
    haystack: Option<&'a [u8]>,
}

impl<'a> Iterator for MemchrSplit<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        let haystack = self.haystack?;
        if let Some(pos) = memchr::memchr(self.needle, haystack) {
            let (front, back) = haystack.split_at(pos);
            self.haystack = Some(&back[1..]);
            Some(front)
        } else {
            self.haystack.take()
        }
    }
}

#[test]
fn test_memchr_split() {
    assert_eq!(
        memchr_split(2, &[]).collect::<Vec<_>>(),
        [&[]] as [&[u8]; 1]
    );
    assert_eq!(
        memchr_split(2, &[2]).collect::<Vec<_>>(),
        [&[], &[]] as [&[u8]; 2]
    );
    assert_eq!(
        memchr_split(2, &[1]).collect::<Vec<_>>(),
        [&[1]] as [&[u8]; 1]
    );
    assert_eq!(
        memchr_split(2, &[1, 2]).collect::<Vec<_>>(),
        [&[1], &[]] as [&[u8]; 2]
    );
    assert_eq!(
        memchr_split(2, &[2, 1]).collect::<Vec<_>>(),
        [&[], &[1]] as [&[u8]; 2]
    );
    assert_eq!(
        memchr_split(2, &[1, 2, 2, 1]).collect::<Vec<_>>(),
        [&[1], &[], &[1]] as [&[u8]; 3]
    );
}

#[test]
fn leading_space_is_not_stripped() {
    let no_leading_space = Event::default().data("\tfoobar");
    assert_eq!(&*no_leading_space.finalize(), b"data:\tfoobar\n\n");

    let leading_space = Event::default().data(" foobar");
    assert_eq!(&*leading_space.finalize(), b"data:  foobar\n\n");
}
