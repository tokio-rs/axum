//! OpenTelemetry middleware.
//!
//! See [`opentelemetry_tracing_layer`] for more details.

use axum::{
    extract::{ConnectInfo, MatchedPath, OriginalUri},
    response::Response,
};
use http::{header, uri::Scheme, Method, Request, Version};
use opentelemetry::trace::TraceContextExt;
use opentelemetry_lib as opentelemetry;
use std::{borrow::Cow, net::SocketAddr, time::Duration};
use tower_http::{
    classify::{ServerErrorsAsFailures, ServerErrorsFailureClass, SharedClassifier},
    trace::{MakeSpan, OnBodyChunk, OnEos, OnFailure, OnRequest, OnResponse, TraceLayer},
};
use tracing::{field::Empty, Span};

/// OpenTelemetry tracing middleware.
///
/// This returns a [`TraceLayer`] configured to use [OpenTelemetry's conventional span field
/// names][otel].
///
/// # Span fields
///
/// The following fields will be set on the span:
///
/// - `http.client_ip`: The client's IP address. Requires using
/// [`Router::into_make_service_with_connect_info`]
/// - `http.flavor`: The protocol version used (http 1.1, http 2.0, etc)
/// - `http.host`: The value of the `Host` header
/// - `http.method`: The request method
/// - `http.route`: The matched route
/// - `http.scheme`: The URI scheme used (`HTTP` or `HTTPS`)
/// - `http.status_code`: The response status code
/// - `http.target`: The full request target including path and query parameters
/// - `http.user_agent`: The value of the `User-Agent` header
/// - `otel.kind`: Always `server`
/// - `otel.status_code`: `OK` if the response is success, `ERROR` if it is a 5xx
/// - `trace_id`: The trace id as tracted via the remote span context.
///
/// # Example
///
/// ```
/// use axum::{Router, routing::get, http::Request};
/// use axum_extra::middleware::opentelemetry_tracing_layer;
/// use std::net::SocketAddr;
/// use tower::ServiceBuilder;
///
/// let app = Router::new()
///     .route("/", get(|| async {}))
///     .layer(opentelemetry_tracing_layer());
///
/// # async {
/// axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
///     // we must use `into_make_service_with_connect_info` for `opentelemetry_tracing_layer` to
///     // access the client ip
///     .serve(app.into_make_service_with_connect_info::<SocketAddr, _>())
///     .await
///     .expect("server failed");
/// # };
/// ```
///
/// # Complete example
///
/// See the "opentelemetry-jaeger" example for a complete setup that includes an OpenTelemetry
/// pipeline sending traces to jaeger.
///
/// [otel]: https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/semantic_conventions/http.md
/// [`Router::into_make_service_with_connect_info`]: axum::Router::into_make_service_with_connect_info
pub fn opentelemetry_tracing_layer() -> TraceLayer<
    SharedClassifier<ServerErrorsAsFailures>,
    OtelMakeSpan,
    OtelOnRequest,
    OtelOnResponse,
    OtelOnBodyChunk,
    OtelOnEos,
    OtelOnFailure,
> {
    TraceLayer::new_for_http()
        .make_span_with(OtelMakeSpan)
        .on_request(OtelOnRequest)
        .on_response(OtelOnResponse)
        .on_body_chunk(OtelOnBodyChunk)
        .on_eos(OtelOnEos)
        .on_failure(OtelOnFailure)
}

/// A [`MakeSpan`] that creates tracing spans using [OpenTelemetry's conventional field names][otel].
///
/// [otel]: https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/semantic_conventions/http.md
#[derive(Clone, Copy, Debug)]
pub struct OtelMakeSpan;

impl<B> MakeSpan<B> for OtelMakeSpan {
    fn make_span(&mut self, req: &Request<B>) -> Span {
        let user_agent = req
            .headers()
            .get(header::USER_AGENT)
            .map_or("", |h| h.to_str().unwrap_or(""));

        let host = req
            .headers()
            .get(header::HOST)
            .map_or("", |h| h.to_str().unwrap_or(""));

        let scheme = req
            .uri()
            .scheme()
            .map_or_else(|| "HTTP".into(), http_scheme);

        let http_route = if let Some(matched_path) = req.extensions().get::<MatchedPath>() {
            matched_path.as_str().to_owned()
        } else if let Some(uri) = req.extensions().get::<OriginalUri>() {
            uri.0.path().to_owned()
        } else {
            req.uri().path().to_owned()
        };

        let uri = if let Some(uri) = req.extensions().get::<OriginalUri>() {
            uri.0.clone()
        } else {
            req.uri().clone()
        };
        let http_target = uri
            .path_and_query()
            .map(|path_and_query| path_and_query.to_string())
            .unwrap_or_else(|| uri.path().to_owned());

        let client_ip = req
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(client_ip)| Cow::from(client_ip.to_string()))
            .unwrap_or_default();

        let remote_context = extract_remote_context(req.headers());
        let remote_span = remote_context.span();
        let span_context = remote_span.span_context();
        let trace_id = span_context
            .is_valid()
            .then(|| Cow::from(span_context.trace_id().to_string()))
            .unwrap_or_default();

        let span = tracing::info_span!(
            "HTTP request",
            http.client_ip = %client_ip,
            http.flavor = %http_flavor(req.version()),
            http.host = %host,
            http.method = %http_method(req.method()),
            http.route = %http_route,
            http.scheme = %scheme,
            http.status_code = Empty,
            http.target = %http_target,
            http.user_agent = %user_agent,
            otel.kind = "server",
            otel.status_code = Empty,
            trace_id = %trace_id,
        );

        tracing_opentelemetry::OpenTelemetrySpanExt::set_parent(&span, remote_context);

        span
    }
}

fn http_method(method: &Method) -> Cow<'static, str> {
    match method {
        &Method::CONNECT => "CONNECT".into(),
        &Method::DELETE => "DELETE".into(),
        &Method::GET => "GET".into(),
        &Method::HEAD => "HEAD".into(),
        &Method::OPTIONS => "OPTIONS".into(),
        &Method::PATCH => "PATCH".into(),
        &Method::POST => "POST".into(),
        &Method::PUT => "PUT".into(),
        &Method::TRACE => "TRACE".into(),
        other => other.to_string().into(),
    }
}

fn http_flavor(version: Version) -> Cow<'static, str> {
    match version {
        Version::HTTP_09 => "0.9".into(),
        Version::HTTP_10 => "1.0".into(),
        Version::HTTP_11 => "1.1".into(),
        Version::HTTP_2 => "2.0".into(),
        Version::HTTP_3 => "3.0".into(),
        other => format!("{:?}", other).into(),
    }
}

fn http_scheme(scheme: &Scheme) -> Cow<'static, str> {
    if scheme == &Scheme::HTTP {
        "http".into()
    } else if scheme == &Scheme::HTTPS {
        "https".into()
    } else {
        scheme.to_string().into()
    }
}

// If remote request has no span data the propagator defaults to an unsampled context
fn extract_remote_context(headers: &http::HeaderMap) -> opentelemetry::Context {
    struct HeaderExtractor<'a>(&'a http::HeaderMap);

    impl<'a> opentelemetry::propagation::Extractor for HeaderExtractor<'a> {
        fn get(&self, key: &str) -> Option<&str> {
            self.0.get(key).and_then(|value| value.to_str().ok())
        }

        fn keys(&self) -> Vec<&str> {
            self.0.keys().map(|value| value.as_str()).collect()
        }
    }

    let extractor = HeaderExtractor(headers);
    opentelemetry::global::get_text_map_propagator(|propagator| propagator.extract(&extractor))
}

/// Callback that [`Trace`] will call when it receives a request.
///
/// [`Trace`]: tower_http::trace::Trace
#[derive(Clone, Copy, Debug)]
pub struct OtelOnRequest;

impl<B> OnRequest<B> for OtelOnRequest {
    #[inline]
    fn on_request(&mut self, _request: &Request<B>, _span: &Span) {}
}

/// Callback that [`Trace`] will call when it receives a response.
///
/// [`Trace`]: tower_http::trace::Trace
#[derive(Clone, Copy, Debug)]
pub struct OtelOnResponse;

impl<B> OnResponse<B> for OtelOnResponse {
    fn on_response(self, response: &Response<B>, _latency: Duration, span: &Span) {
        let status = response.status().as_u16().to_string();
        span.record("http.status_code", &tracing::field::display(status));

        // assume there is no error, if there is `OtelOnFailure` will be called and override this
        span.record("otel.status_code", &"OK");
    }
}

/// Callback that [`Trace`] will call when the response body produces a chunk.
///
/// [`Trace`]: tower_http::trace::Trace
#[derive(Clone, Copy, Debug)]
pub struct OtelOnBodyChunk;

impl<B> OnBodyChunk<B> for OtelOnBodyChunk {
    #[inline]
    fn on_body_chunk(&mut self, _chunk: &B, _latency: Duration, _span: &Span) {}
}

/// Callback that [`Trace`] will call when a streaming response completes.
///
/// [`Trace`]: tower_http::trace::Trace
#[derive(Clone, Copy, Debug)]
pub struct OtelOnEos;

impl OnEos for OtelOnEos {
    #[inline]
    fn on_eos(self, _trailers: Option<&http::HeaderMap>, _stream_duration: Duration, _span: &Span) {
    }
}

/// Callback that [`Trace`] will call when a response or end-of-stream is classified as a failure.
///
/// [`Trace`]: tower_http::trace::Trace
#[derive(Clone, Copy, Debug)]
pub struct OtelOnFailure;

impl OnFailure<ServerErrorsFailureClass> for OtelOnFailure {
    fn on_failure(&mut self, failure: ServerErrorsFailureClass, _latency: Duration, span: &Span) {
        match failure {
            ServerErrorsFailureClass::StatusCode(status) => {
                if status.is_server_error() {
                    span.record("otel.status_code", &"ERROR");
                }
            }
            ServerErrorsFailureClass::Error(_) => {
                span.record("otel.status_code", &"ERROR");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_json_diff::assert_json_include;
    use axum::{body::Body, routing::get, Router};
    use http::{Request, StatusCode};
    use serde_json::{json, Value};
    use std::{
        convert::TryInto,
        sync::mpsc::{self, Receiver, SyncSender},
    };
    use tower::{Service, ServiceExt};
    use tracing_subscriber::{
        fmt::{format::FmtSpan, MakeWriter},
        util::SubscriberInitExt,
        EnvFilter,
    };

    #[tokio::test]
    async fn correct_fields_on_span_for_http() {
        let svc = Router::new()
            .route("/", get(|| async { StatusCode::OK }))
            .route(
                "/users/:id",
                get(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
            )
            .layer(opentelemetry_tracing_layer());

        let [(root_new, root_close), (users_id_new, users_id_close)] = spans_for_requests(
            svc,
            [
                Request::builder()
                    .header("user-agent", "tests")
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
                Request::builder()
                    .uri("/users/123")
                    .body(Body::empty())
                    .unwrap(),
            ],
        )
        .await;

        assert_json_include!(
            actual: root_new,
            expected: json!({
                "fields": {
                    "message": "new",
                },
                "level": "INFO",
                "span": {
                    "http.client_ip": "",
                    "http.flavor": "1.1",
                    "http.host": "",
                    "http.method": "GET",
                    "http.route": "/",
                    "http.scheme": "HTTP",
                    "http.target": "/",
                    "http.user_agent": "tests",
                    "name": "HTTP request",
                    "otel.kind": "server",
                    "trace_id": ""
                }
            }),
        );

        assert_json_include!(
            actual: root_close,
            expected: json!({
                "fields": {
                    "message": "close",
                },
                "level": "INFO",
                "span": {
                    "http.client_ip": "",
                    "http.flavor": "1.1",
                    "http.host": "",
                    "http.method": "GET",
                    "http.route": "/",
                    "http.scheme": "HTTP",
                    "http.status_code": "200",
                    "http.target": "/",
                    "http.user_agent": "tests",
                    "name": "HTTP request",
                    "otel.kind": "server",
                    "otel.status_code": "OK",
                    "trace_id": ""
                }
            }),
        );

        assert_json_include!(
            actual: users_id_new,
            expected: json!({
                "span": {
                    "http.route": "/users/:id",
                    "http.target": "/users/123",
                }
            }),
        );

        assert_json_include!(
            actual: users_id_close,
            expected: json!({
                "span": {
                    "http.status_code": "500",
                    "otel.status_code": "ERROR",
                }
            }),
        );
    }

    async fn spans_for_requests<const N: usize>(
        mut router: Router<Body>,
        reqs: [Request<Body>; N],
    ) -> [(Value, Value); N] {
        use axum::body::HttpBody as _;

        let (make_writer, rx) = duplex_writer();
        let subscriber = tracing_subscriber::fmt::fmt()
            .json()
            .with_env_filter(EnvFilter::try_new("axum_extra=trace").unwrap())
            .with_writer(make_writer)
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
            .finish();
        let _guard = subscriber.set_default();

        let mut spans = Vec::new();

        for req in reqs {
            let mut res = router.ready().await.unwrap().call(req).await.unwrap();

            while res.data().await.is_some() {}
            res.trailers().await.unwrap();
            drop(res);

            let logs = std::iter::from_fn(|| rx.try_recv().ok())
                .map(|bytes| serde_json::from_slice::<Value>(&bytes).unwrap())
                .collect::<Vec<_>>();

            let [new, close]: [_; 2] = logs.try_into().unwrap();

            spans.push((new, close));
        }

        spans.try_into().unwrap()
    }

    fn duplex_writer() -> (DuplexWriter, Receiver<Vec<u8>>) {
        let (tx, rx) = mpsc::sync_channel(1024);
        (DuplexWriter { tx }, rx)
    }

    #[derive(Clone)]
    struct DuplexWriter {
        tx: SyncSender<Vec<u8>>,
    }

    impl<'a> MakeWriter<'a> for DuplexWriter {
        type Writer = Self;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    impl std::io::Write for DuplexWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.tx.send(buf.to_vec()).unwrap();
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}
