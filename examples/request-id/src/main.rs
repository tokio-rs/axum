//! Run with
//!
//! ```not_rust
//! cargo run -p example-request-id
//! ```

use axum::{http::Request, response::Html, routing::get, Router};
use tower::ServiceBuilder;
use tower_http::{
    request_id::{MakeRequestUuid, RequestId},
    trace::TraceLayer,
    ServiceBuilderExt,
};
use tracing::{info, info_span};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // axum logs rejections from built-in extractors with the `axum::rejection`
                // target, at `TRACE` level. `axum::rejection=trace` enables showing those events
                format!(
                    "{}=debug,tower_http=debug,axum::rejection=trace",
                    env!("CARGO_CRATE_NAME")
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let middleware = ServiceBuilder::new()
        .set_x_request_id(MakeRequestUuid)
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                // Log the request id as generated.
                let request_id = request.extensions().get::<RequestId>().unwrap();

                match request_id.header_value().to_str() {
                    Ok(request_id) => info_span!("http_request", request_id),
                    Err(_) => info_span!("http_request", request_id = ?request_id),
                }
            }),
        )
        // send headers from request to response headers
        .propagate_x_request_id();

    // build our application with a route
    let app = Router::new().route("/", get(handler)).layer(middleware);

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await;
}

async fn handler() -> Html<&'static str> {
    info!("Hello world!");
    Html("<h1>Hello, World!</h1>")
}
