use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    routing::get,
    serve::{Handshaker, Listener},
    Router,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
};
use tokio_rustls::{
    rustls::{
        pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
        ServerConfig,
    },
    TlsAcceptor,
};

#[derive(Clone)]
struct RustlsHandshaker(TlsAcceptor);

impl<L> Handshaker<L> for RustlsHandshaker
where
    L: Listener,
    L::Io: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Io = tokio_rustls::server::TlsStream<L::Io>;
    type Error = std::io::Error;
    type Future = tokio_rustls::Accept<L::Io>;

    fn handshake(&self, io: L::Io) -> Self::Future {
        self.0.accept(io)
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let rustls_config = rustls_server_config(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("key.pem"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("cert.pem"),
    );

    let tls_acceptor = TlsAcceptor::from(rustls_config);

    let app = Router::new().route("/", get(|| async { "Hello, World!" }));

    let listener = TcpListener::bind(("127.0.0.1", 3000)).await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app)
        .with_handshaker(RustlsHandshaker(tls_acceptor))
        .await
        .unwrap();
}

fn rustls_server_config(key: impl AsRef<Path>, cert: impl AsRef<Path>) -> Arc<ServerConfig> {
    let key = PrivateKeyDer::from_pem_file(key).unwrap();

    let certs = CertificateDer::pem_file_iter(cert)
        .unwrap()
        .map(|cert| cert.unwrap())
        .collect();

    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .expect("bad certificate/key");

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Arc::new(config)
}
