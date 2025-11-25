use axum::{routing::get, serve::TlsListener, Router};
use std::path::{Path, PathBuf};
use tokio::net::TcpListener;
use tokio_rustls::{
    rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
    rustls::ServerConfig,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=trace", env!("CARGO_CRATE_NAME")).into()),
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

    let bind = "127.0.0.1:8443";
    let tcp_listener = TcpListener::bind(bind).await.unwrap();
    info!("HTTPS server listening on {bind}. To contact curl --insecure https://localhost:8443");
    let app = Router::new().route("/", get(|| async { " Hello from HTTPS" }));

    let tls_listener = TlsListener::new(tcp_listener, rustls_config);

    axum::serve(tls_listener, app.into_make_service()).await
}

fn rustls_server_config(key: impl AsRef<Path>, cert: impl AsRef<Path>) -> ServerConfig {
    let key = PrivateKeyDer::from_pem_file(key).unwrap();

    let certs = CertificateDer::pem_file_iter(cert)
        .unwrap()
        .map(|cert| cert.unwrap())
        .collect();

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .expect("bad certificate/key")
}
