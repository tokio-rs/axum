use openssl::ssl::{Ssl, SslAcceptor, SslFiletype, SslMethod};
use tokio_openssl::SslStream;

use axum::{extract::ConnectInfo, routing::get, Router};
use futures_util::future::poll_fn;
use hyper::server::{
    accept::Accept,
    conn::{AddrIncoming, Http},
};
use std::{net::SocketAddr, path::PathBuf, pin::Pin, sync::Arc};
use tokio::net::TcpListener;
use tower::MakeService;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_low_level_openssl=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut tls_builder = SslAcceptor::mozilla_modern_v5(SslMethod::tls()).unwrap();

    tls_builder
        .set_certificate_file(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("self_signed_certs")
                .join("cert.pem"),
            SslFiletype::PEM,
        )
        .unwrap();

    tls_builder
        .set_private_key_file(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("self_signed_certs")
                .join("key.pem"),
            SslFiletype::PEM,
        )
        .unwrap();

    tls_builder.check_private_key().unwrap();

    let acceptor = tls_builder.build();

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    let mut listener = AddrIncoming::from_listener(listener).unwrap();

    let protocol = Arc::new(Http::new());

    let mut app = Router::new()
        .route("/", get(handler))
        .into_make_service_with_connect_info::<SocketAddr>();

    tracing::info!("listening on https://localhost:3000");

    loop {
        let stream = poll_fn(|cx| Pin::new(&mut listener).poll_accept(cx))
            .await
            .unwrap()
            .unwrap();

        let acceptor = acceptor.clone();

        let protocol = protocol.clone();

        let svc = app.make_service(&stream);

        tokio::spawn(async move {
            let ssl = Ssl::new(acceptor.context()).unwrap();
            let mut tls_stream = SslStream::new(ssl, stream).unwrap();

            SslStream::accept(Pin::new(&mut tls_stream)).await.unwrap();

            let _ = protocol
                .serve_connection(tls_stream, svc.await.unwrap())
                .await;
        });
    }
}

async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
    addr.to_string()
}
