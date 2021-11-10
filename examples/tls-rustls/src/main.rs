//! Run with
//!
//! ```not_rust
//! cargo run -p example-tls-rustls
//! ```

use axum::{routing::get, Router};
use axum_server::tls_rustls::RustlsConfig;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "example_tls_rustls=debug")
    }
    tracing_subscriber::fmt::init();

    let config = RustlsConfig::from_pem_file(
        "examples/tls-rustls/self_signed_certs/cert.pem",
        "examples/tls-rustls/self_signed_certs/key.pem",
    )
    .await
    .unwrap();

    let app = Router::new().route("/", get(handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler() -> &'static str {
    "Hello, World!"
}
