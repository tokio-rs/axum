//! Run jaeger
//!
//! ```not_rust
//! docker run -d -p6831:6831/udp -p6832:6832/udp -p16686:16686 -p14268:14268 jaegertracing/all-in-one:latest
//! ```
//!
//! Run the server
//!
//! ```not_rust
//! cargo run -p example-opentelemetry-jaeger
//! ```

use axum::{http::Request, routing::get, Router};
use axum_extra::middleware::opentelemtry_tracing_layer;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::request_id::{RequestId, SetRequestIdLayer};
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() {
    // set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var(
            "RUST_LOG",
            "axum_extra=debug,example_opentelemetry_jaeger=debug,tower_http=debug",
        )
    }

    // start an otel jaeger trace pipeline
    opentelemetry::global::set_text_map_propagator(
        opentelemetry::sdk::propagation::TraceContextPropagator::new(),
    );

    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("axum-example")
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();

    // create a `tracing-subscriber` using `tracing-opentelemetry` and logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap())
        .init();

    // build our application with a route
    let app = Router::new().route("/", get(handler)).layer(
        ServiceBuilder::new()
            .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
            .layer(opentelemtry_tracing_layer()),
    );

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr, _>())
        .await
        .unwrap();

    opentelemetry::global::shutdown_tracer_provider();
}

async fn handler() -> &'static str {
    "Hello, World!"
}

#[derive(Clone, Copy)]
struct MakeRequestUuid;

impl tower_http::request_id::MakeRequestId for MakeRequestUuid {
    fn make_request_id<B>(&mut self, _: &Request<B>) -> Option<RequestId> {
        let request_id = uuid::Uuid::new_v4().to_string().parse().ok()?;
        Some(RequestId::new(request_id))
    }
}
