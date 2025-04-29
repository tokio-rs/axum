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
//! use futures_util::stream::{self, Stream};
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
//! # let _: Router = app;
//! ```

use crate::{
    body::{Bytes, HttpBody},
    BoxError,
};
use axum_core::{
    body::Body,
    response::{IntoResponse, Response},
};
use bytes::{BufMut, BytesMut};
use futures_util::stream::{Stream, TryStream};
use http_body::Frame;
use pin_project_lite::pin_project;
use std::{
    fmt, mem,
    pin::Pin,
    task::{ready, Context, Poll},
    time::Duration,
};
use sync_wrapper::SyncWrapper;

/// An SSE response
#[derive(Clone)]
#[must_use]
pub struct Sse<S> {
    stream: S,
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
        Sse { stream }
    }

    /// Configure the interval between keep-alive messages.
    ///
    /// Defaults to no keep-alive messages.
    #[cfg(feature = "tokio")]
    pub fn keep_alive(self, keep_alive: KeepAlive) -> Sse<KeepAliveStream<S>> {
        Sse {
            stream: KeepAliveStream::new(keep_alive, self.stream),
        }
    }
}

impl<S> fmt::Debug for Sse<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sse")
            .field("stream", &format_args!("{}", std::any::type_name::<S>()))
            .finish()
    }
}

impl<S, E> IntoResponse for Sse<S>
where
    S: Stream<Item = Result<Event, E>> + Send + 'static,
    E: Into<BoxError>,
{
    fn into_response(self) -> Response {
        (
            [
                (http::header::CONTENT_TYPE, mime::TEXT_EVENT_STREAM.as_ref()),
                (http::header::CACHE_CONTROL, "no-cache"),
            ],
            Body::new(SseBody {
                event_stream: SyncWrapper::new(self.stream),
            }),
        )
            .into_response()
    }
}

pin_project! {
    struct SseBody<S> {
        #[pin]
        event_stream: SyncWrapper<S>,
    }
}

impl<S, E> HttpBody for SseBody<S>
where
    S: Stream<Item = Result<Event, E>>,
{
    type Data = Bytes;
    type Error = E;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();

        match ready!(this.event_stream.get_pin_mut().poll_next(cx)) {
            Some(Ok(event)) => Poll::Ready(Some(Ok(Frame::data(event.finalize())))),
            Some(Err(error)) => Poll::Ready(Some(Err(error))),
            None => Poll::Ready(None),
        }
    }
}

/// The state of an event's buffer.
///
/// This type allows creating events in a `const` context
/// by using a finalized buffer.
///
/// While the buffer is active, more bytes can be written to it.
/// Once finalized, it's immutable and cheap to clone.
/// The buffer is active during the event building, but eventually
/// becomes finalized to send http body frames as [`Bytes`].
#[derive(Debug, Clone)]
enum Buffer {
    Active(BytesMut),
    Finalized(Bytes),
}

impl Buffer {
    /// Returns a mutable reference to the internal buffer.
    ///
    /// If the buffer was finalized, this method creates
    /// a new active buffer with the previous contents.
    fn as_mut(&mut self) -> &mut BytesMut {
        match self {
            Buffer::Active(bytes_mut) => bytes_mut,
            Buffer::Finalized(bytes) => {
                *self = Buffer::Active(BytesMut::from(mem::take(bytes)));
                match self {
                    Buffer::Active(bytes_mut) => bytes_mut,
                    Buffer::Finalized(_) => unreachable!(),
                }
            }
        }
    }
}

/// Server-sent event
#[derive(Debug, Clone)]
#[must_use]
pub struct Event {
    buffer: Buffer,
    flags: EventFlags,
}

impl Event {
    /// Default keep-alive event
    pub const DEFAULT_KEEP_ALIVE: Self = Self::finalized(Bytes::from_static(b":\n\n"));

    const fn finalized(bytes: Bytes) -> Self {
        Self {
            buffer: Buffer::Finalized(bytes),
            flags: EventFlags::from_bits(0),
        }
    }

    /// Set the event's data data field(s) (`data: <content>`)
    ///
    /// Newlines in `data` will automatically be broken across `data: ` fields.
    ///
    /// This corresponds to [`MessageEvent`'s data field].
    ///
    /// Note that events with an empty data field will be ignored by the browser.
    ///
    /// # Panics
    ///
    /// - Panics if `data` contains any carriage returns, as they cannot be transmitted over SSE.
    /// - Panics if `data` or `json_data` have already been called.
    ///
    /// [`MessageEvent`'s data field]: https://developer.mozilla.org/en-US/docs/Web/API/MessageEvent/data
    pub fn data<T>(mut self, data: T) -> Event
    where
        T: AsRef<str>,
    {
        if self.flags.contains(EventFlags::HAS_DATA) {
            panic!("Called `Event::data` multiple times");
        }

        for line in memchr_split(b'\n', data.as_ref().as_bytes()) {
            self.field("data", line);
        }

        self.flags.insert(EventFlags::HAS_DATA);

        self
    }

    /// Set the event's data field to a value serialized as unformatted JSON (`data: <content>`).
    ///
    /// This corresponds to [`MessageEvent`'s data field].
    ///
    /// # Panics
    ///
    /// Panics if `data` or `json_data` have already been called.
    ///
    /// [`MessageEvent`'s data field]: https://developer.mozilla.org/en-US/docs/Web/API/MessageEvent/data
    #[cfg(feature = "json")]
    pub fn json_data<T>(mut self, data: T) -> Result<Event, axum_core::Error>
    where
        T: serde::Serialize,
    {
        struct IgnoreNewLines<'a>(bytes::buf::Writer<&'a mut BytesMut>);
        impl std::io::Write for IgnoreNewLines<'_> {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                let mut last_split = 0;
                for delimiter in memchr::memchr2_iter(b'\n', b'\r', buf) {
                    self.0.write_all(&buf[last_split..delimiter])?;
                    last_split = delimiter + 1;
                }
                self.0.write_all(&buf[last_split..])?;
                Ok(buf.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                self.0.flush()
            }
        }
        if self.flags.contains(EventFlags::HAS_DATA) {
            panic!("Called `Event::json_data` multiple times");
        }

        let buffer = self.buffer.as_mut();
        buffer.extend_from_slice(b"data: ");
        serde_json::to_writer(IgnoreNewLines(buffer.writer()), &data)
            .map_err(axum_core::Error::new)?;
        buffer.put_u8(b'\n');

        self.flags.insert(EventFlags::HAS_DATA);

        Ok(self)
    }

    /// Set the event's comment field (`:<comment-text>`).
    ///
    /// This field will be ignored by most SSE clients.
    ///
    /// Unlike other functions, this function can be called multiple times to add many comments.
    ///
    /// # Panics
    ///
    /// Panics if `comment` contains any newlines or carriage returns, as they are not allowed in
    /// comments.
    pub fn comment<T>(mut self, comment: T) -> Event
    where
        T: AsRef<str>,
    {
        self.field("", comment.as_ref());
        self
    }

    /// Set the event's name field (`event:<event-name>`).
    ///
    /// This corresponds to the `type` parameter given when calling `addEventListener` on an
    /// [`EventSource`]. For example, `.event("update")` should correspond to
    /// `.addEventListener("update", ...)`. If no event type is given, browsers will fire a
    /// [`message` event] instead.
    ///
    /// [`EventSource`]: https://developer.mozilla.org/en-US/docs/Web/API/EventSource
    /// [`message` event]: https://developer.mozilla.org/en-US/docs/Web/API/EventSource/message_event
    ///
    /// # Panics
    ///
    /// - Panics if `event` contains any newlines or carriage returns.
    /// - Panics if this function has already been called on this event.
    pub fn event<T>(mut self, event: T) -> Event
    where
        T: AsRef<str>,
    {
        if self.flags.contains(EventFlags::HAS_EVENT) {
            panic!("Called `Event::event` multiple times");
        }
        self.flags.insert(EventFlags::HAS_EVENT);

        self.field("event", event.as_ref());

        self
    }

    /// Set the event's retry timeout field (`retry:<timeout>`).
    ///
    /// This sets how long clients will wait before reconnecting if they are disconnected from the
    /// SSE endpoint. Note that this is just a hint: clients are free to wait for longer if they
    /// wish, such as if they implement exponential backoff.
    ///
    /// # Panics
    ///
    /// Panics if this function has already been called on this event.
    pub fn retry(mut self, duration: Duration) -> Event {
        if self.flags.contains(EventFlags::HAS_RETRY) {
            panic!("Called `Event::retry` multiple times");
        }
        self.flags.insert(EventFlags::HAS_RETRY);

        let buffer = self.buffer.as_mut();
        buffer.extend_from_slice(b"retry:");

        let secs = duration.as_secs();
        let millis = duration.subsec_millis();

        if secs > 0 {
            // format seconds
            buffer.extend_from_slice(itoa::Buffer::new().format(secs).as_bytes());

            // pad milliseconds
            if millis < 10 {
                buffer.extend_from_slice(b"00");
            } else if millis < 100 {
                buffer.extend_from_slice(b"0");
            }
        }

        // format milliseconds
        buffer.extend_from_slice(itoa::Buffer::new().format(millis).as_bytes());

        buffer.put_u8(b'\n');

        self
    }

    /// Set the event's identifier field (`id:<identifier>`).
    ///
    /// This corresponds to [`MessageEvent`'s `lastEventId` field]. If no ID is in the event itself,
    /// the browser will set that field to the last known message ID, starting with the empty
    /// string.
    ///
    /// [`MessageEvent`'s `lastEventId` field]: https://developer.mozilla.org/en-US/docs/Web/API/MessageEvent/lastEventId
    ///
    /// # Panics
    ///
    /// - Panics if `id` contains any newlines, carriage returns or null characters.
    /// - Panics if this function has already been called on this event.
    pub fn id<T>(mut self, id: T) -> Event
    where
        T: AsRef<str>,
    {
        if self.flags.contains(EventFlags::HAS_ID) {
            panic!("Called `Event::id` multiple times");
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

        let buffer = self.buffer.as_mut();
        buffer.extend_from_slice(name.as_bytes());
        buffer.put_u8(b':');
        buffer.put_u8(b' ');
        buffer.extend_from_slice(value);
        buffer.put_u8(b'\n');
    }

    fn finalize(self) -> Bytes {
        match self.buffer {
            Buffer::Finalized(bytes) => bytes,
            Buffer::Active(mut bytes_mut) => {
                bytes_mut.put_u8(b'\n');
                bytes_mut.freeze()
            }
        }
    }
}

impl Default for Event {
    fn default() -> Self {
        Self {
            buffer: Buffer::Active(BytesMut::new()),
            flags: EventFlags::from_bits(0),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
struct EventFlags(u8);

impl EventFlags {
    const HAS_DATA: Self = Self::from_bits(0b0001);
    const HAS_EVENT: Self = Self::from_bits(0b0010);
    const HAS_RETRY: Self = Self::from_bits(0b0100);
    const HAS_ID: Self = Self::from_bits(0b1000);

    const fn bits(&self) -> u8 {
        self.0
    }

    const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    const fn contains(&self, other: Self) -> bool {
        self.bits() & other.bits() == other.bits()
    }

    fn insert(&mut self, other: Self) {
        *self = Self::from_bits(self.bits() | other.bits());
    }
}

/// Configure the interval between keep-alive messages, the content
/// of each message, and the associated stream.
#[derive(Debug, Clone)]
#[must_use]
pub struct KeepAlive {
    event: Event,
    max_interval: Duration,
}

impl KeepAlive {
    /// Create a new `KeepAlive`.
    pub fn new() -> Self {
        Self {
            event: Event::DEFAULT_KEEP_ALIVE,
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
    pub fn text<I>(self, text: I) -> Self
    where
        I: AsRef<str>,
    {
        self.event(Event::default().comment(text))
    }

    /// Customize the event of the keep-alive message.
    ///
    /// Default is an empty comment.
    ///
    /// # Panics
    ///
    /// Panics if `event` contains any newline or carriage returns, as they are not allowed in SSE
    /// comments.
    pub fn event(mut self, event: Event) -> Self {
        self.event = Event::finalized(event.finalize());
        self
    }
}

impl Default for KeepAlive {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "tokio")]
pin_project! {
    /// A wrapper around a stream that produces keep-alive events
    #[derive(Debug)]
    pub struct KeepAliveStream<S> {
        #[pin]
        alive_timer: tokio::time::Sleep,
        #[pin]
        inner: S,
        keep_alive: KeepAlive,
    }
}

#[cfg(feature = "tokio")]
impl<S> KeepAliveStream<S> {
    fn new(keep_alive: KeepAlive, inner: S) -> Self {
        Self {
            alive_timer: tokio::time::sleep(keep_alive.max_interval),
            inner,
            keep_alive,
        }
    }

    fn reset(self: Pin<&mut Self>) {
        let this = self.project();
        this.alive_timer
            .reset(tokio::time::Instant::now() + this.keep_alive.max_interval);
    }
}

#[cfg(feature = "tokio")]
impl<S, E> Stream for KeepAliveStream<S>
where
    S: Stream<Item = Result<Event, E>>,
{
    type Item = Result<Event, E>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use std::future::Future;

        let mut this = self.as_mut().project();

        match this.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(event))) => {
                self.reset();

                Poll::Ready(Some(Ok(event)))
            }
            Poll::Ready(Some(Err(error))) => Poll::Ready(Some(Err(error))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => {
                ready!(this.alive_timer.poll(cx));

                let event = this.keep_alive.event.clone();

                self.reset();

                Poll::Ready(Some(Ok(event)))
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{routing::get, test_helpers::*, Router};
    use futures_util::stream;
    use serde_json::value::RawValue;
    use std::{collections::HashMap, convert::Infallible};
    use tokio_stream::StreamExt as _;

    #[test]
    fn leading_space_is_not_stripped() {
        let no_leading_space = Event::default().data("\tfoobar");
        assert_eq!(&*no_leading_space.finalize(), b"data: \tfoobar\n\n");

        let leading_space = Event::default().data(" foobar");
        assert_eq!(&*leading_space.finalize(), b"data:  foobar\n\n");
    }

    #[test]
    fn valid_json_raw_value_chars_stripped() {
        let json_string = "{\r\"foo\":  \n\r\r   \"bar\\n\"\n}";
        let json_raw_value_event = Event::default()
            .json_data(serde_json::from_str::<&RawValue>(json_string).unwrap())
            .unwrap();
        assert_eq!(
            &*json_raw_value_event.finalize(),
            format!("data: {}\n\n", json_string.replace(['\n', '\r'], "")).as_bytes()
        );
    }

    #[crate::test]
    async fn basic() {
        let app = Router::new().route(
            "/",
            get(|| async {
                let stream = stream::iter(vec![
                    Event::default().data("one").comment("this is a comment"),
                    Event::default()
                        .json_data(serde_json::json!({ "foo": "bar" }))
                        .unwrap(),
                    Event::default()
                        .event("three")
                        .retry(Duration::from_secs(30))
                        .id("unique-id"),
                ])
                .map(Ok::<_, Infallible>);
                Sse::new(stream)
            }),
        );

        let client = TestClient::new(app);
        let mut stream = client.get("/").await;

        assert_eq!(stream.headers()["content-type"], "text/event-stream");
        assert_eq!(stream.headers()["cache-control"], "no-cache");

        let event_fields = parse_event(&stream.chunk_text().await.unwrap());
        assert_eq!(event_fields.get("data").unwrap(), "one");
        assert_eq!(event_fields.get("comment").unwrap(), "this is a comment");

        let event_fields = parse_event(&stream.chunk_text().await.unwrap());
        assert_eq!(event_fields.get("data").unwrap(), "{\"foo\":\"bar\"}");
        assert!(!event_fields.contains_key("comment"));

        let event_fields = parse_event(&stream.chunk_text().await.unwrap());
        assert_eq!(event_fields.get("event").unwrap(), "three");
        assert_eq!(event_fields.get("retry").unwrap(), "30000");
        assert_eq!(event_fields.get("id").unwrap(), "unique-id");
        assert!(!event_fields.contains_key("comment"));

        assert!(stream.chunk_text().await.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn keep_alive() {
        const DELAY: Duration = Duration::from_secs(5);

        let app = Router::new().route(
            "/",
            get(|| async {
                let stream = stream::repeat_with(|| Event::default().data("msg"))
                    .map(Ok::<_, Infallible>)
                    .throttle(DELAY);

                Sse::new(stream).keep_alive(
                    KeepAlive::new()
                        .interval(Duration::from_secs(1))
                        .text("keep-alive-text"),
                )
            }),
        );

        let client = TestClient::new(app);
        let mut stream = client.get("/").await;

        for _ in 0..5 {
            // first message should be an event
            let event_fields = parse_event(&stream.chunk_text().await.unwrap());
            assert_eq!(event_fields.get("data").unwrap(), "msg");

            // then 4 seconds of keep-alive messages
            for _ in 0..4 {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let event_fields = parse_event(&stream.chunk_text().await.unwrap());
                assert_eq!(event_fields.get("comment").unwrap(), "keep-alive-text");
            }
        }
    }

    #[tokio::test(start_paused = true)]
    async fn keep_alive_ends_when_the_stream_ends() {
        const DELAY: Duration = Duration::from_secs(5);

        let app = Router::new().route(
            "/",
            get(|| async {
                let stream = stream::repeat_with(|| Event::default().data("msg"))
                    .map(Ok::<_, Infallible>)
                    .throttle(DELAY)
                    .take(2);

                Sse::new(stream).keep_alive(
                    KeepAlive::new()
                        .interval(Duration::from_secs(1))
                        .text("keep-alive-text"),
                )
            }),
        );

        let client = TestClient::new(app);
        let mut stream = client.get("/").await;

        // first message should be an event
        let event_fields = parse_event(&stream.chunk_text().await.unwrap());
        assert_eq!(event_fields.get("data").unwrap(), "msg");

        // then 4 seconds of keep-alive messages
        for _ in 0..4 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let event_fields = parse_event(&stream.chunk_text().await.unwrap());
            assert_eq!(event_fields.get("comment").unwrap(), "keep-alive-text");
        }

        // then the last event
        let event_fields = parse_event(&stream.chunk_text().await.unwrap());
        assert_eq!(event_fields.get("data").unwrap(), "msg");

        // then no more events or keep-alive messages
        assert!(stream.chunk_text().await.is_none());
    }

    fn parse_event(payload: &str) -> HashMap<String, String> {
        let mut fields = HashMap::new();

        let mut lines = payload.lines().peekable();
        while let Some(line) = lines.next() {
            if line.is_empty() {
                assert!(lines.next().is_none());
                break;
            }

            let (mut key, value) = line.split_once(':').unwrap();
            let value = value.trim();
            if key.is_empty() {
                key = "comment";
            }
            fields.insert(key.to_owned(), value.to_owned());
        }

        fields
    }

    #[test]
    fn memchr_splitting() {
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
}
