//! Run with
//!
//! ```not_rust
//! cargo run -p example-tls-rustls
//! ```

// NOTE: This example is currently broken since axum-server requires `S: Sync`,
// that isn't necessary and will be fixed in a future release

fn main() {}

// use axum::{handler::get, Router};

// #[tokio::main]
// async fn main() {
//     // Set the RUST_LOG, if it hasn't been explicitly defined
//     if std::env::var_os("RUST_LOG").is_none() {
//         std::env::set_var("RUST_LOG", "example_tls_rustls=debug")
//     }
//     tracing_subscriber::fmt::init();

//     // let app = Router::new().route("/", get(handler));

//     // axum_server::bind_rustls("127.0.0.1:3000")
//     //     .private_key_file("examples/tls-rustls/self_signed_certs/key.pem")
//     //     .certificate_file("examples/tls-rustls/self_signed_certs/cert.pem")
//     //     .serve(app)
//     //     .await
//     //     .unwrap();
// }

// async fn handler() -> &'static str {
//     "Hello, World!"
// }
