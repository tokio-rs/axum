//! Run with
//!
//! ```not_rust
//! cargo run -p example-sse
//! ```

use axum::extract::TypedHeader;
use axum::{
    response::sse::{Event, Sse},
    routing::get,
    Router,
};
use futures::stream::{self, Stream};
use std::net::SocketAddr;
use std::str::FromStr;
use std::{convert::Infallible, path::PathBuf, time::Duration};
use tokio_stream::StreamExt as _;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_sse=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

    let static_files_service = ServeDir::new(assets_dir).append_index_html_on_directories(true);

    // build our application with a route
    let app = Router::new()
        .fallback_service(static_files_service)
        .route("/sse", get(sse_handler))
        .layer(TraceLayer::new_for_http());

    // run it
    let address = SocketAddr::from_str("127.0.0.1:3000").unwrap();
    tracing::debug!("listening on {}", address);

    axum::Server::bind(&address)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn sse_handler(
    TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    println!("`{}` connected", user_agent.as_str());

    // A `Stream` that repeats an event every second
    //
    // You can also create streams from tokio channels using the wrappers in
    // https://docs.rs/tokio-stream
    let mut curr = 0;
    let stream = stream::repeat_with(move || {
        curr += 1;
        // SSE supports only UTF-8. Therefore, conversion is required
        Event::default().data(curr.to_string())
    })
    .map(Ok)
    .throttle(Duration::from_secs(1));

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}
