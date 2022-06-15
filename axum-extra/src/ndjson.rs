//! Newline delimited JSON extractor and response.

use axum::{
    async_trait,
    body::{HttpBody, StreamBody},
    extract::{rejection::BodyAlreadyExtracted, FromRequest, RequestParts},
    response::{IntoResponse, Response},
    BoxError,
};
use bytes::{BufMut, Bytes, BytesMut};
use futures_util::stream::{BoxStream, Stream, TryStream, TryStreamExt};
use http::header::CONTENT_TYPE;
use pin_project_lite::pin_project;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    io::{self, Write},
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::AsyncBufReadExt;
use tokio_stream::wrappers::LinesStream;
use tokio_util::io::StreamReader;

pin_project! {
    /// A stream of newline delimited JSON ([NDJSON]).
    ///
    /// This can be used both as an extractor and as a response.
    ///
    /// # As extractor
    ///
    /// ```rust
    /// use axum_extra::ndjson::NdJson;
    /// use futures::stream::StreamExt;
    ///
    /// async fn handler(mut stream: NdJson<serde_json::Value>) {
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
    /// use axum_extra::ndjson::NdJson;
    /// use futures::stream::Stream;
    ///
    /// fn stream_of_values() -> impl Stream<Item = Result<serde_json::Value, BoxError>> {
    ///     # futures::stream::empty()
    /// }
    ///
    /// async fn handler() -> Response {
    ///     NdJson::new(stream_of_values()).into_response()
    /// }
    /// ```
    ///
    /// [NDJSON]: https://ndjson.org/
    // we use `AsExtractor` as the default because you're more likely to name this type if its used
    // as an extractor
    pub struct NdJson<S, T = AsExtractor> {
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
        Extrator {
            #[pin]
            stream: BoxStream<'static, Result<S, axum::Error>>,
        },
    }
}

/// Maker type used to prove that an `NdJson` was constructed via `FromRequest`.
#[derive(Debug)]
#[non_exhaustive]
pub struct AsExtractor;

/// Maker type used to prove that an `NdJson` was constructed via `NdJson::new`.
#[derive(Debug)]
#[non_exhaustive]
pub struct AsResponse;

impl<S> NdJson<S, AsResponse> {
    /// Create a new `NdJson` from a stream of items.
    pub fn new(stream: S) -> Self {
        Self {
            inner: Inner::Response { stream },
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<B, T> FromRequest<B> for NdJson<T, AsExtractor>
where
    B: HttpBody + Send + 'static,
    B::Data: Into<Bytes>,
    B::Error: Into<BoxError>,
    T: DeserializeOwned,
{
    type Rejection = BodyAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        // `Stream::lines` isn't a thing so we have to convert it into an `AsyncRead`
        // so we can call `AsyncRead::lines` and then convert it back to a `Stream`

        let body = req.take_body().ok_or_else(BodyAlreadyExtracted::default)?;
        let body = BodyStream { body };

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
            inner: Inner::Extrator {
                stream: Box::pin(deserialized_stream),
            },
            _marker: PhantomData,
        })
    }
}

// like `axum::extract::BodyStream` except it doesn't box the inner body
// we don't need that since we box the final stream in `Inner::Extrator`
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

impl<T> Stream for NdJson<T, AsExtractor> {
    type Item = Result<T, axum::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.project().inner.project() {
            InnerProj::Extrator { stream } => stream.poll_next(cx),
            // `NdJson<_, AsExtractor>` can only be constructed via `FromRequest`
            // which doesn't use this variant
            InnerProj::Response { .. } => unreachable!(),
        }
    }
}

impl<S> IntoResponse for NdJson<S, AsResponse>
where
    S: TryStream + Send + 'static,
    S::Ok: Serialize + Send,
    S::Error: Into<BoxError>,
{
    fn into_response(self) -> Response {
        let inner = match self.inner {
            Inner::Response { stream } => stream,
            // `NdJson<_, AsResponse>` can only be constructed via `NdJson::new`
            // which doesn't use this variant
            Inner::Extrator { .. } => unreachable!(),
        };

        let stream = inner.map_err(Into::into).and_then(|value| async move {
            let mut buf = BytesMut::new().writer();
            serde_json::to_writer(&mut buf, &value)?;
            buf.write_all(b"\n")?;
            Ok::<_, BoxError>(buf.into_inner().freeze())
        });
        let stream = StreamBody::new(stream);

        ([(CONTENT_TYPE, "application/x-ndjson")], stream).into_response()
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
            post(|mut stream: NdJson<User>| async move {
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
                vec![
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
                NdJson::new(values)
            }),
        );

        let client = TestClient::new(app);

        let res = client.get("/").send().await;
        assert_eq!(res.headers()["content-type"], "application/x-ndjson");

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
