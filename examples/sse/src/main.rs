//! Run with
//!
//! ```not_rust
//! cargo run -p example-sse
//! ```
//! Test with
//! ```not_rust
//! cargo test -p example-sse
//! ```

use axum::{
    extract::Query,
    response::sse::{Event, Sse},
    routing::get,
    Router,
};
use axum_extra::{headers, TypedHeader};
use futures::stream::{self, Stream};
use serde::Deserialize;
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

    // build our application
    let app = app();

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

fn app() -> Router {
    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let static_files_service = ServeDir::new(assets_dir).append_index_html_on_directories(true);
    // build our application with a route
    Router::new()
        .fallback_service(static_files_service)
        .route("/sse", get(sse_handler))
        .layer(TraceLayer::new_for_http())
}

#[derive(Deserialize)]
struct Repeat {
    take_n: Option<usize>,
}

async fn sse_handler(
    TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
    repeat: Query<Repeat>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    println!("`{}` connected", user_agent.as_str());
    // default to repeating 100 times
    let n = repeat.take_n.unwrap_or(100);

    // A `Stream` that repeats an event every second
    //
    // You can also create streams from tokio channels using the wrappers in
    // https://docs.rs/tokio-stream
    let stream = stream::repeat_with(|| Event::default().data("hi!"))
        .take(n)
        .map(Ok)
        .throttle(Duration::from_secs(1));

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}

#[cfg(test)]
mod tests {
    use eventsource_stream::Eventsource;
    use tokio::net::TcpListener;

    use super::*;

    #[tokio::test]
    async fn integration_test() {
        // A helper function that spawns our application in the background
        async fn spawn_app(host: impl Into<String>) -> String {
            let _host = host.into();
            // Bind to localhost at the port 0, which will let the OS assign an available port to us
            let listener = TcpListener::bind(format!("{}:0", _host)).await.unwrap();
            // Retrieve the port assigned to us by the OS
            let port = listener.local_addr().unwrap().port();
            tokio::spawn(async move {
                let app = app();
                axum::serve(listener, app).await.unwrap();
            });
            // Returns address (e.g. http://127.0.0.1{random_port})
            format!("http://{}:{}", _host, port)
        }
        let listening_url = spawn_app("127.0.0.1").await;

        let take_n = 1;
        let mut event_stream = reqwest::Client::new()
            .get(&format!("{}/sse?take_n={}", listening_url, take_n))
            .header("User-Agent", "integration_test")
            .send()
            .await
            .unwrap()
            .bytes_stream()
            .eventsource();

        let mut event_data: Vec<String> = vec![];
        while let Some(event) = event_stream.next().await {
            match event {
                Ok(event) => {
                    // break the loop at the end of SSE stream
                    if event.data == "[DONE]" {
                        break;
                    }

                    event_data.push(event.data);
                }
                Err(_) => {
                    panic!("Error in event stream");
                }
            }
        }

        assert!(event_data.len() == take_n);
        assert!(event_data[0] == "hi!");
    }
}
