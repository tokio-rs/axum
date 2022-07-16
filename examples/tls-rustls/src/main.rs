//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-tls-rustls
//! ```

use axum::{
    extract::Host,
    handler::Handler,
    http::Uri,
    response::Redirect,
    routing::get,
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use std::{net::SocketAddr, path::PathBuf};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "example_tls_rustls=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // optional: spawn a second server to redirect http requests to this server
    tokio::spawn(redirect());

    // configure certificate and private key used by https
    let config = RustlsConfig::from_pem_file(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("cert.pem"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("key.pem"),
    )
    .await
    .unwrap();

    let app = Router::new().route("/", get(handler));

    // run https server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler() -> &'static str {
    "Hello, World!"
}

async fn redirect() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 7878));
    tracing::debug!("http redirect listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(fallback.into_make_service())
        .await
        .unwrap()
}

async fn fallback(Host(host): Host, uri: Uri) -> Redirect {
    tracing::debug!("308: Permanent Redirect");

    // Can remove call to `.replace()` if using default
    // ports for http (80) and https (443).
    Redirect::permanent(&*format!(
        "https://{}{}",
        &host.replace("7878", "3000"),
        &uri
    ))
}
