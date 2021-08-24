//! Run with
//!
//! ```not_rust
//! cargo run -p example-tls-rustls
//! ```

use axum::{handler::get, Router};

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "example_tls_rustls=debug")
    }
    tracing_subscriber::fmt::init();

    let app = Router::new().route("/", get(handler));

    axum_server::bind_rustls("127.0.0.1:3000")
        .private_key_file("examples/tls-rustls/self_signed_certs/key.pem")
        .certificate_file("examples/tls-rustls/self_signed_certs/cert.pem")
        .serve(app)
        .await
        .unwrap();
}

async fn handler() -> &'static str {
    "Hello, World!"
}
