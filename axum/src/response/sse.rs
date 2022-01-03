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
    body::{self, Bytes, HttpBody},
    response::{IntoResponse, Response},
    BoxError,
};
use futures_util::{
    ready,
    stream::{Stream, TryStream},
};
use pin_project_lite::pin_project;
use std::{
    borrow::Cow,
    fmt,
    fmt::Write,
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
                    keep_alive
                        .poll_event(cx)
                        .map(|e| Some(Ok(Bytes::from(e.to_string()))))
                } else {
                    Poll::Pending
                }
            }
            Poll::Ready(Some(Ok(event))) => {
                if let Some(keep_alive) = this.keep_alive.as_pin_mut() {
                    keep_alive.reset();
                }
                Poll::Ready(Some(Ok(Bytes::from(event.to_string()))))
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

/// Server-sent event
#[derive(Default, Debug)]
pub struct Event {
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
    #[cfg(feature = "json")]
    Json(String),
}

impl Event {
    /// Set the event's data data field(s) (`data:<content>`)
    ///
    /// Newlines in `data` will automatically be broken across `data:` fields.
    ///
    /// This corresponds to [`MessageEvent`'s data field].
    ///
    /// [`MessageEvent`'s data field]: https://developer.mozilla.org/en-US/docs/Web/API/MessageEvent/data
    ///
    /// # Panics
    ///
    /// Panics if `data` contains any carriage returns, as they cannot be transmitted over SSE.
    pub fn data<T>(mut self, data: T) -> Event
    where
        T: Into<String>,
    {
        let data = data.into();
        assert_eq!(
            memchr::memchr(b'\r', data.as_bytes()),
            None,
            "SSE data cannot contain carriage returns",
        );
        self.data = Some(DataType::Text(data));
        self
    }

    /// Set the event's data field to a value serialized as unformatted JSON (`data:<content>`).
    ///
    /// This corresponds to [`MessageEvent`'s data field].
    ///
    /// [`MessageEvent`'s data field]: https://developer.mozilla.org/en-US/docs/Web/API/MessageEvent/data
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    pub fn json_data<T>(mut self, data: T) -> Result<Event, serde_json::Error>
    where
        T: serde::Serialize,
    {
        self.data = Some(DataType::Json(serde_json::to_string(&data)?));
        Ok(self)
    }

    /// Set the event's comment field (`:<comment-text>`).
    ///
    /// This field will be ignored by most SSE clients.
    ///
    /// # Panics
    ///
    /// Panics if `comment` contains any newlines or carriage returns, as they are not allowed in
    /// comments.
    pub fn comment<T>(mut self, comment: T) -> Event
    where
        T: Into<String>,
    {
        let comment = comment.into();
        assert_eq!(
            memchr::memchr2(b'\r', b'\n', comment.as_bytes()),
            None,
            "SSE comment cannot contain newlines or carriage returns"
        );
        self.comment = Some(comment);
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
    /// Panics if `event` contains any newlines or carriage returns.
    pub fn event<T>(mut self, event: T) -> Event
    where
        T: Into<String>,
    {
        let event = event.into();
        assert_eq!(
            memchr::memchr2(b'\r', b'\n', event.as_bytes()),
            None,
            "SSE event name cannot contain newlines or carriage returns"
        );
        self.event = Some(event);
        self
    }

    /// Set the event's retry timeout field (`retry:<timeout>`).
    ///
    /// This sets how long clients will wait before reconnecting if they are disconnected from the
    /// SSE endpoint. Note that this is just a hint: clients are free to wait for longer if they
    /// wish, such as if they implement exponential backoff.
    pub fn retry(mut self, duration: Duration) -> Event {
        self.retry = Some(duration);
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
    /// Panics if `id` contains any newlines, carriage returns or null characters.
    pub fn id<T>(mut self, id: T) -> Event
    where
        T: Into<String>,
    {
        let id = id.into();
        assert_eq!(
            memchr::memchr3(b'\r', b'\n', b'\0', id.as_bytes()),
            None,
            "Event ID cannot contain newlines, carriage returns or null characters",
        );
        self.id = Some(id);
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
            "event: ".fmt(f)?;
            event.fmt(f)?;
            f.write_char('\n')?;
        }

        match &self.data {
            Some(DataType::Text(data)) => {
                for line in data.split('\n') {
                    "data: ".fmt(f)?;
                    line.fmt(f)?;
                    f.write_char('\n')?;
                }
            }
            #[cfg(feature = "json")]
            Some(DataType::Json(data)) => {
                "data:".fmt(f)?;
                data.fmt(f)?;
                f.write_char('\n')?;
            }
            None => {}
        }

        if let Some(id) = &self.id {
            "id: ".fmt(f)?;
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

    fn poll_event(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Event> {
        let this = self.as_mut().project();

        ready!(this.alive_timer.poll(cx));

        let comment_str = this.keep_alive.comment_text.clone();
        let event = Event::default().comment(comment_str);

        self.reset();

        Poll::Ready(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{routing::get, test_helpers::*, Router};
    use futures::stream;
    use std::{collections::HashMap, convert::Infallible};
    use tokio_stream::StreamExt as _;

    #[test]
    fn leading_space_is_not_stripped() {
        let no_leading_space = Event::default().data("\tfoobar");
        assert_eq!(no_leading_space.to_string(), "data: \tfoobar\n\n");

        let leading_space = Event::default().data(" foobar");
        assert_eq!(leading_space.to_string(), "data:  foobar\n\n");
    }

    #[tokio::test]
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
        let mut stream = client.get("/").send().await;

        assert_eq!(stream.headers()["content-type"], "text/event-stream");
        assert_eq!(stream.headers()["cache-control"], "no-cache");

        let event_fields = parse_event(&stream.chunk_text().await.unwrap());
        assert_eq!(event_fields.get("data").unwrap(), "one");
        assert_eq!(event_fields.get("comment").unwrap(), "this is a comment");

        let event_fields = parse_event(&stream.chunk_text().await.unwrap());
        assert_eq!(event_fields.get("data").unwrap(), "{\"foo\":\"bar\"}");
        assert!(event_fields.get("comment").is_none());

        let event_fields = parse_event(&stream.chunk_text().await.unwrap());
        assert_eq!(event_fields.get("event").unwrap(), "three");
        assert_eq!(event_fields.get("retry").unwrap(), "30000");
        assert_eq!(event_fields.get("id").unwrap(), "unique-id");
        assert!(event_fields.get("comment").is_none());

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
        let mut stream = client.get("/").send().await;

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
        let mut stream = client.get("/").send().await;

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
}
