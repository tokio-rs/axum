//! Run with
//!
//! ```not_rust
//! cargo run --example sse --features=headers
//! ```

use axum::{
    extract::TypedHeader,
    handler::get,
    response::sse::{sse, Event, Sse},
    route,
    routing::{nest, RoutingDsl},
};
use futures::stream::{self, Stream};
use http::StatusCode;
use std::{convert::Infallible, net::SocketAddr, time::Duration};
use tokio_stream::StreamExt as _;
use tower_http::{services::ServeDir, trace::TraceLayer};

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "sse=debug,tower_http=debug")
    }
    tracing_subscriber::fmt::init();

    let static_files_service =
        axum::service::get(ServeDir::new("examples/sse").append_index_html_on_directories(true))
            .handle_error(|error: std::io::Error| {
                Ok::<_, std::convert::Infallible>((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unhandled internal error: {}", error),
                ))
            });

    // build our application with a route
    let app = nest("/", static_files_service)
        .route("/sse", get(sse_handler))
        .layer(TraceLayer::new_for_http());

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn sse_handler(
    TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    println!("`{}` connected", user_agent.as_str());

    // A `Stream` that repeats an event every second
    let stream = stream::repeat_with(|| Event::default().data("hi!"))
        .map(Ok)
        .throttle(Duration::from_secs(1));

    sse(stream)
}
