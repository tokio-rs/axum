//! Newline delimited JSON extractor and response.

use axum::{
    async_trait,
    body::{HttpBody, StreamBody},
    extract::FromRequest,
    response::{IntoResponse, Response},
    BoxError,
};
use bytes::{BufMut, Bytes, BytesMut};
use futures_util::stream::{BoxStream, Stream, TryStream, TryStreamExt};
use http::Request;
use pin_project_lite::pin_project;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    convert::Infallible,
    io::{self, Write},
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::AsyncBufReadExt;
use tokio_stream::wrappers::LinesStream;
use tokio_util::io::StreamReader;

pin_project! {
    /// A stream of newline delimited JSON.
    ///
    /// This can be used both as an extractor and as a response.
    ///
    /// # As extractor
    ///
    /// ```rust
    /// use axum_extra::json_lines::JsonLines;
    /// use futures_util::stream::StreamExt;
    ///
    /// async fn handler(mut stream: JsonLines<serde_json::Value>) {
    ///     while let Some(value) = stream.next().await {
    ///         // ...
    ///     }
    /// }
    /// ```
    ///
    /// # As response
    ///
    /// ```rust
    /// use axum::{BoxError, response::{IntoResponse, Response}};
    /// use axum_extra::json_lines::JsonLines;
    /// use futures_util::stream::Stream;
    ///
    /// fn stream_of_values() -> impl Stream<Item = Result<serde_json::Value, BoxError>> {
    ///     # futures_util::stream::empty()
    /// }
    ///
    /// async fn handler() -> Response {
    ///     JsonLines::new(stream_of_values()).into_response()
    /// }
    /// ```
    // we use `AsExtractor` as the default because you're more likely to name this type if its used
    // as an extractor
    #[must_use]
    pub struct JsonLines<S, T = AsExtractor> {
        #[pin]
        inner: Inner<S>,
        _marker: PhantomData<T>,
    }
}

pin_project! {
    #[project = InnerProj]
    enum Inner<S> {
        Response {
            #[pin]
            stream: S,
        },
        Extractor {
            #[pin]
            stream: BoxStream<'static, Result<S, axum::Error>>,
        },
    }
}

/// Maker type used to prove that an `JsonLines` was constructed via `FromRequest`.
#[derive(Debug)]
#[non_exhaustive]
pub struct AsExtractor;

/// Maker type used to prove that an `JsonLines` was constructed via `JsonLines::new`.
#[derive(Debug)]
#[non_exhaustive]
pub struct AsResponse;

impl<S> JsonLines<S, AsResponse> {
    /// Create a new `JsonLines` from a stream of items.
    pub fn new(stream: S) -> Self {
        Self {
            inner: Inner::Response { stream },
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<S, B, T> FromRequest<S, B> for JsonLines<T, AsExtractor>
where
    B: HttpBody + Send + 'static,
    B::Data: Into<Bytes>,
    B::Error: Into<BoxError>,
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: Request<B>, _state: &S) -> Result<Self, Self::Rejection> {
        // `Stream::lines` isn't a thing so we have to convert it into an `AsyncRead`
        // so we can call `AsyncRead::lines` and then convert it back to a `Stream`
        let body = BodyStream {
            body: req.into_body(),
        };

        let stream = body
            .map_ok(Into::into)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err));
        let read = StreamReader::new(stream);
        let lines_stream = LinesStream::new(read.lines());

        let deserialized_stream =
            lines_stream
                .map_err(axum::Error::new)
                .and_then(|value| async move {
                    serde_json::from_str::<T>(&value).map_err(axum::Error::new)
                });

        Ok(Self {
            inner: Inner::Extractor {
                stream: Box::pin(deserialized_stream),
            },
            _marker: PhantomData,
        })
    }
}

// like `axum::extract::BodyStream` except it doesn't box the inner body
// we don't need that since we box the final stream in `Inner::Extractor`
pin_project! {
    struct BodyStream<B> {
        #[pin]
        body: B,
    }
}

impl<B> Stream for BodyStream<B>
where
    B: HttpBody + Send + 'static,
{
    type Item = Result<B::Data, B::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().body.poll_data(cx)
    }
}

impl<T> Stream for JsonLines<T, AsExtractor> {
    type Item = Result<T, axum::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.project().inner.project() {
            InnerProj::Extractor { stream } => stream.poll_next(cx),
            // `JsonLines<_, AsExtractor>` can only be constructed via `FromRequest`
            // which doesn't use this variant
            InnerProj::Response { .. } => unreachable!(),
        }
    }
}

impl<S> IntoResponse for JsonLines<S, AsResponse>
where
    S: TryStream + Send + 'static,
    S::Ok: Serialize + Send,
    S::Error: Into<BoxError>,
{
    fn into_response(self) -> Response {
        let inner = match self.inner {
            Inner::Response { stream } => stream,
            // `JsonLines<_, AsResponse>` can only be constructed via `JsonLines::new`
            // which doesn't use this variant
            Inner::Extractor { .. } => unreachable!(),
        };

        let stream = inner.map_err(Into::into).and_then(|value| async move {
            let mut buf = BytesMut::new().writer();
            serde_json::to_writer(&mut buf, &value)?;
            buf.write_all(b"\n")?;
            Ok::<_, BoxError>(buf.into_inner().freeze())
        });
        let stream = StreamBody::new(stream);

        // there is no consensus around mime type yet
        // https://github.com/wardi/jsonlines/issues/36
        stream.into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{
        routing::{get, post},
        Router,
    };
    use futures_util::StreamExt;
    use http::StatusCode;
    use serde::Deserialize;
    use std::{convert::Infallible, error::Error};

    #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
    struct User {
        id: i32,
    }

    #[tokio::test]
    async fn extractor() {
        let app = Router::new().route(
            "/",
            post(|mut stream: JsonLines<User>| async move {
                assert_eq!(stream.next().await.unwrap().unwrap(), User { id: 1 });
                assert_eq!(stream.next().await.unwrap().unwrap(), User { id: 2 });
                assert_eq!(stream.next().await.unwrap().unwrap(), User { id: 3 });

                // sources are downcastable to `serde_json::Error`
                let err = stream.next().await.unwrap().unwrap_err();
                let _: &serde_json::Error = err
                    .source()
                    .unwrap()
                    .downcast_ref::<serde_json::Error>()
                    .unwrap();
            }),
        );

        let client = TestClient::new(app);

        let res = client
            .post("/")
            .body(
                [
                    "{\"id\":1}",
                    "{\"id\":2}",
                    "{\"id\":3}",
                    // to trigger an error for source downcasting
                    "{\"id\":false}",
                ]
                .join("\n"),
            )
            .send()
            .await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn response() {
        let app = Router::new().route(
            "/",
            get(|| async {
                let values = futures_util::stream::iter(vec![
                    Ok::<_, Infallible>(User { id: 1 }),
                    Ok::<_, Infallible>(User { id: 2 }),
                    Ok::<_, Infallible>(User { id: 3 }),
                ]);
                JsonLines::new(values)
            }),
        );

        let client = TestClient::new(app);

        let res = client.get("/").send().await;

        let values = res
            .text()
            .await
            .lines()
            .map(|line| serde_json::from_str::<User>(line).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            values,
            vec![User { id: 1 }, User { id: 2 }, User { id: 3 },]
        );
    }
}
