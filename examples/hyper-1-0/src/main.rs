//! Run with
//!
//! ```not_rust
//! cargo run -p example-hyper-1-0
//! ```

use axum::{routing::get, Router};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tower_hyper_http_body_compat::{
    HttpBody1ToHttpBody04, TowerService03HttpServiceAsHyper1HttpService,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// this is hyper 1.0
use hyper::{body::Incoming, server::conn::http1};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_hyper_1_0=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // you have to use `HttpBody1ToHttpBody04<Incoming>` as the second type parameter to `Router`
    let app: Router<_, HttpBody1ToHttpBody04<Incoming>> = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        // we can still add regular tower middleware
        .layer(TraceLayer::new_for_http());

    // `Router` implements tower-service 0.3's `Service` trait. Convert that to something
    // that implements hyper 1.0's `Service` trait.
    let service = TowerService03HttpServiceAsHyper1HttpService::new(app);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let tcp_listener = TcpListener::bind(addr).await.unwrap();
    tracing::debug!("listening on {addr}");
    loop {
        let (tcp_stream, _) = tcp_listener.accept().await.unwrap();
        let tcp_stream = hyper_util::rt::TokioIo::new(tcp_stream);
        let service = service.clone();
        tokio::task::spawn(async move {
            if let Err(http_err) = http1::Builder::new()
                .keep_alive(true)
                .serve_connection(tcp_stream, service)
                .await
            {
                eprintln!("Error while serving HTTP connection: {http_err}");
            }
        });
    }
}
