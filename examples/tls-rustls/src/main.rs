//! Run with
//!
//! ```not_rust
//! cargo run -p example-tls-rustls
//! ```

use axum::{handler::get, Router};

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "example_tls_rustls=debug")
    }
    tracing_subscriber::fmt::init();

    // this doesn't currently work because axum-server requires services to be
    // `Sync`. That requirement can be removed but requires making a new
    // release

    // let app = Router::new().route("/", get(handler));

    // axum_server::bind_rustls("127.0.0.1:3000")
    //     .private_key_file("examples/tls-rustls/self_signed_certs/key.pem")
    //     .certificate_file("examples/tls-rustls/self_signed_certs/cert.pem")
    //     .serve(app)
    //     .await
    //     .unwrap();
}

async fn handler() -> &'static str {
    "Hello, World!"
}
