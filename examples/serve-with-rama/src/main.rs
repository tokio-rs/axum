//! Run with
//!
//! ```not_rust
//! cargo run -p example-serve-with-rama
//! ```
//!
//! This example shows how to run axum using rama as the HTTP driving server instead of the default
//! hyper.
use std::convert::Infallible;
use std::future::{ready, Future};

use axum::body::{Body as AxumBody, HttpBody};
use axum::http::StatusCode;
use axum::response::Response;
use axum::serve::{Connection, ConnectionBuilder};
use axum::{extract::Request, routing::get, Router};
use pin_project_lite::pin_project;
use rama::graceful::ShutdownBuilder;
use rama::http::core::body::Frame;
use rama::http::core::server::conn::auto;
use rama::http::Body as RamaBody;
use rama::rt::Executor;
use rama::utils::tower::ServiceAdapter;
use rama::Context;
use tokio::sync::watch;
use tower::{Service, ServiceExt};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(|| ready(StatusCode::IM_A_TEAPOT)));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    let connection_builder = RamaConnectionBuilder::new();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve::serve(listener, app)
        .with_connection_builder(connection_builder)
        .await
        .unwrap();
}

#[derive(Clone)]
pub struct RamaConnectionBuilder {
    server: rama::http::server::HttpServer<auto::Builder>,
    shutdown: Option<watch::Receiver<()>>,
}

pin_project! {
    pub struct RamaConnection<F> {
        #[pin]
        inner: F
    }
}

pin_project! {
    pub struct SyncBody<B> {
        #[pin]
        inner: B,
    }
}

// SAFETY
// This is fine because we never provide references to the inner body and the only publicly
// accessible method using it requires `Pin<&mut Self>` which means exclusive access.
unsafe impl<B: Send> Sync for SyncBody<B> {}

impl<B> SyncBody<B> {
    pub fn new(body: B) -> Self {
        Self { inner: body }
    }
}

impl<B: HttpBody> HttpBody for SyncBody<B> {
    type Data = B::Data;

    type Error = B::Error;

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        self.project().inner.poll_frame(cx)
    }

    // We must NOT delegate the provided methods to the inner body. We must NOT use references to
    // the inner body as it may not be `Sync` and who knows what thread holds references to it.
}

impl RamaConnectionBuilder {
    fn new() -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(());
        let shutdown = ShutdownBuilder::new()
            .with_signal(async move { shutdown_tx.closed().await })
            .build();
        Self {
            server: rama::http::server::HttpServer::auto(Executor::graceful(shutdown.guard())),
            shutdown: Some(shutdown_rx),
        }
    }
}

impl<Io, S> ConnectionBuilder<Io, S> for RamaConnectionBuilder
where
    Io: rama::net::stream::Stream,
    S: Clone + Send + Sync + 'static,
    S: Service<Request, Response = Response, Error = Infallible, Future: Send>
        + Clone
        + Send
        + Sync
        + 'static,
{
    fn build_connection(&mut self, io: Io, service: S) -> impl Connection {
        let rama_service = ServiceAdapter::new(
            service
                .map_request(|request: Request<RamaBody>| request.map(AxumBody::new))
                .map_response(|response: Response<AxumBody>| {
                    response.map(SyncBody::new).map(RamaBody::new)
                }),
        );
        RamaConnection {
            inner: self.server.serve(Context::default(), io, rama_service),
        }
    }

    fn graceful_shutdown(&mut self) {
        self.shutdown.take();
    }
}

impl<F> Connection for RamaConnection<F>
where
    F: Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send,
{
    fn poll_connection(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        self.project().inner.poll(cx)
    }
}
