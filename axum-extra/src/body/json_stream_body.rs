use axum::body::HttpBody;
use axum::response::{IntoResponse, Response};
use futures_util::stream::BoxStream;
use futures_util::StreamExt;
use http::HeaderMap;
use serde::Serialize;
use std::fmt::Formatter;
use std::pin::Pin;
use std::task::{Context, Poll};

/// JSON streaming body support that [`HttpBody`] created from an async ['Stream'] of
/// any serializable structure.
///
/// [JSON Streaming](https://en.wikipedia.org/wiki/JSON_streaming) is a term referring to streaming a
/// stream of element as independent JSON objects as a continuous HTTP request or response.
///
/// This type of responses are useful when you are reading huge stream of objects from some source (such as database, file, etc)
/// and want to avoid huge memory allocations to store on the server side.
///
/// The implementation streams objects as a normal JSON array with proper delimiters,
/// so any kind of clients even without JSON streaming support is able to read the response.
///
/// # Example
///
/// `AsyncReadBody` can be used to stream the contents of a file:
///
/// ```rust
/// use futures_util::stream::BoxStream;
/// use axum::{
///     Router,
///     routing::get,
///     http::{StatusCode, header::CONTENT_TYPE},
///     response::{Response, IntoResponse},
/// };
/// use axum_extra::body::JsonStreamBody;
/// use serde::Serialize;
///
/// #[derive(Debug, Clone, Serialize)]
/// struct MyTestStructure {
///     some_test_field: String
/// }
///
/// // Your possibly stream of objects
/// fn my_source_stream() -> BoxStream<'static, MyTestStructure> {
///     // Simulating a stream with a plain vector and throttling to show how it works
///     use tokio_stream::StreamExt;
///     Box::pin(futures::stream::iter(vec![
///         MyTestStructure {
///             some_test_field: "test1".to_string()
///         }; 1000
///     ]).throttle(std::time::Duration::from_millis(50)))
/// }
///
/// // Route implementation:
/// async fn test_json_stream() -> impl IntoResponse {
///     JsonStreamBody::new(my_source_stream())
/// }
///
/// let app = Router::new().route("/test-json-stream", get(test_json_stream));
/// # let _: Router = app;
/// ```

pub struct JsonStreamBody<'a> {
    stream: BoxStream<'a, Result<axum::body::Bytes, axum::Error>>,
}

impl<'a> std::fmt::Debug for JsonStreamBody<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "JsonStreamBody")
    }
}

impl<'a> JsonStreamBody<'a> {
    /// Create a new `JsonStreamBody` providing a stream of your objects.
    pub fn new<T>(stream: BoxStream<'a, T>) -> Self
    where
        T: Serialize + Send + Sync + 'a,
    {
        Self {
            stream: Self::json_stream_to_bytes(stream),
        }
    }

    fn json_stream_to_bytes<T>(
        stream: BoxStream<'a, T>,
    ) -> BoxStream<Result<axum::body::Bytes, axum::Error>>
    where
        T: Serialize + Send + Sync + 'a,
    {
        let stream_bytes: BoxStream<Result<axum::body::Bytes, axum::Error>> = Box::pin({
            stream.enumerate().map(|(index, obj)| {
                let mut output = vec![];
                serde_json::to_vec::<T>(&obj)
                    .map(|obj_vec| {
                        if index != 0 {
                            output.extend(Self::JSON_ARRAY_SEP_BYTES.clone())
                        }
                        output.extend(obj_vec);
                        axum::body::Bytes::from(output)
                    })
                    .map_err(Self::json_err_to_axum)
            })
        });

        let prepend_stream: BoxStream<Result<axum::body::Bytes, axum::Error>> = Box::pin(
            futures_util::stream::once(futures_util::future::ready(Ok::<_, axum::Error>(
                axum::body::Bytes::from(Self::JSON_ARRAY_BEGIN_BYTES.clone()),
            ))),
        );

        let append_stream: BoxStream<Result<axum::body::Bytes, axum::Error>> =
            Box::pin(futures_util::stream::once(futures_util::future::ready(
                Ok::<_, axum::Error>(axum::body::Bytes::from(Self::JSON_ARRAY_END_BYTES.clone())),
            )));

        Box::pin(prepend_stream.chain(stream_bytes.chain(append_stream)))
    }

    fn json_err_to_axum(err: serde_json::Error) -> axum::Error {
        axum::Error::new(err)
    }

    const JSON_ARRAY_BEGIN_BYTES: &'static [u8] = "[".as_bytes();
    const JSON_ARRAY_END_BYTES: &'static [u8] = "]".as_bytes();
    const JSON_ARRAY_SEP_BYTES: &'static [u8] = ",".as_bytes();
}

impl IntoResponse for JsonStreamBody<'static> {
    fn into_response(self) -> Response {
        Response::new(axum::body::boxed(self))
    }
}

impl<'a> HttpBody for JsonStreamBody<'a> {
    type Data = axum::body::Bytes;
    type Error = axum::Error;

    fn poll_data(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        use futures_util::Stream;
        Pin::new(&mut self.stream).poll_next(cx)
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        let mut header_map = HeaderMap::new();
        header_map.insert(
            http::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        Poll::Ready(Ok(Some(header_map)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{routing::*, Router};
    use futures_util::stream;

    #[tokio::test]
    async fn deserialize_json_stream() {
        #[derive(Debug, Clone, Serialize)]
        struct TestOutputStructure {
            foo: String,
        }

        let test_stream_vec = vec![
            TestOutputStructure {
                foo: "bar".to_string()
            };
            100
        ];

        let test_stream = Box::pin(stream::iter(test_stream_vec.clone()));

        let app = Router::new().route("/", get(|| async { JsonStreamBody::new(test_stream) }));

        let client = TestClient::new(app);

        let expected_json = serde_json::to_string(&test_stream_vec).unwrap();
        let res = client.get("/").send().await;
        let body = res.text().await;

        assert_eq!(body, expected_json);
    }
}
