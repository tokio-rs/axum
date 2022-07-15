//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p http-redirect
//! ```

// Run an axum server listening on HTTP_PORT and a rustls axum_server
// listening on HTTPS_PORT. Redirect http request for
// "http://SERVER:HTTP_PORT/" to "https://SERVER:HTTPS_PORT/"
//
// Built off the "tls-rustls" example code.

use axum::{
    extract::Host,
    handler::Handler,
    http::Uri,
    response::{Html, Redirect},
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
            std::env::var("RUST_LOG").unwrap_or_else(|_| "http_redirect=trace".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // spawn http redirect server
    tokio::spawn(redirect());

    // build our application with a route
    let app = Router::new().route("/", get(handler));

    // configure the certificate and private key
    // for use by HTTPS
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

    // run https server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello secured world!</h1>")
}

async fn redirect() {
    let redirect_app = Router::new().fallback(fallback.into_service());

    let addr = SocketAddr::from(([127, 0, 0, 1], 7878));
    tracing::debug!("http redirect listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(redirect_app.into_make_service())
        .await
        .unwrap()
}

async fn fallback(Host(host): Host, uri: Uri) -> Redirect {
    tracing::debug!("308: Permanent Redirect");

    // If using default ports for http (80) and https (443),
    // can remove call to `.replace()`.
    Redirect::permanent(&*format!(
        "https://{}{}",
        &host.replace("7878", "3000"),
        &uri
    ))
}
