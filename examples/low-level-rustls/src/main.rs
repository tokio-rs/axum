//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-low-level-rustls
//! ```

use axum::{extract::ConnectInfo, routing::get, Router};
use futures_util::future::poll_fn;
use hyper::server::{
    accept::Accept,
    conn::{AddrIncoming, Http},
};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::{
    fs::File,
    io::BufReader,
    net::SocketAddr,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};
use tokio::net::TcpListener;
use tokio_rustls::{
    rustls::{Certificate, PrivateKey, ServerConfig},
    TlsAcceptor,
};
use tower::MakeService;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_tls_rustls=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let rustls_config = rustls_server_config(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("key.pem"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("cert.pem"),
    );

    let acceptor = TlsAcceptor::from(rustls_config);

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    let mut listener = AddrIncoming::from_listener(listener).unwrap();

    let protocol = Arc::new(Http::new());

    let mut app = Router::new()
        .route("/", get(handler))
        .into_make_service_with_connect_info::<SocketAddr>();

    loop {
        let stream = poll_fn(|cx| Pin::new(&mut listener).poll_accept(cx))
            .await
            .unwrap()
            .unwrap();

        let acceptor = acceptor.clone();

        let protocol = protocol.clone();

        let svc = app.make_service(&stream);

        tokio::spawn(async move {
            if let Ok(stream) = acceptor.accept(stream).await {
                let _ = protocol.serve_connection(stream, svc.await.unwrap()).await;
            }
        });
    }
}

async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
    addr.to_string()
}

fn rustls_server_config(key: impl AsRef<Path>, cert: impl AsRef<Path>) -> Arc<ServerConfig> {
    let mut key_reader = BufReader::new(File::open(key).unwrap());
    let mut cert_reader = BufReader::new(File::open(cert).unwrap());

    let key = PrivateKey(pkcs8_private_keys(&mut key_reader).unwrap().remove(0));
    let certs = certs(&mut cert_reader)
        .unwrap()
        .into_iter()
        .map(Certificate)
        .collect();

    let mut config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .expect("bad certificate/key");

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Arc::new(config)
}
