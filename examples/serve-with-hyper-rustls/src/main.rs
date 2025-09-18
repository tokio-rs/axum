//! Run with
//!
//! ```not_rust
//! cargo run -p example-serve-with-hyper-rustls
//! ```
//!
//! Test that the server runs with
//! ```bash
//! curl -kv https://localhost:3000
//! ```

use std::convert::Infallible;
use std::error::Error as StdError;
use std::future::poll_fn;
use std::net::{Ipv4Addr, SocketAddr};
use std::pin::pin;
use std::sync::Arc;
use std::{fs, io};

use axum::response::Response;
use axum::serve::{Connection, ConnectionBuilder, Hyper};
use axum::{extract::Request, routing::get, Router};
use http_body::Body as HttpBody;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tower::Service;

#[derive(Clone)]
pub struct HyperRustls {
    tls_acceptor: TlsAcceptor,
    inner: Hyper,
}

impl HyperRustls {
    pub fn try_new() -> anyhow::Result<Self> {
        // Load public certificate.
        let certs = load_certs(&format!(
            "{}/self_signed_certs/cert.pem",
            env!("CARGO_MANIFEST_DIR")
        ))?;
        // Load private key.
        let key = load_private_key(&format!(
            "{}/self_signed_certs/key.pem",
            env!("CARGO_MANIFEST_DIR")
        ))?;

        // Build TLS configuration.
        let mut server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .inspect_err(|e| tracing::error!(error = display(e), "Cannot load certificate."))
            .unwrap();
        server_config.alpn_protocols =
            vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()];

        Ok(Self {
            tls_acceptor: TlsAcceptor::from(Arc::new(server_config)),
            inner: Hyper::default(),
        })
    }
}

impl<Io, S, B> ConnectionBuilder<Io, S> for HyperRustls
where
    Io: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    S: Service<Request, Response = Response<B>, Error = Infallible> + Clone + Send + 'static,
    <S as Service<Request>>::Future: Send,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    fn build_connection(&mut self, io: Io, service: S) -> impl Connection {
        let tls_acceptor = self.tls_acceptor.clone();
        let mut hyper = self.inner.clone();

        Box::pin(async move {
            let tls_stream = match tls_acceptor.accept(io).await {
                Ok(tls_stream) => tls_stream,
                Err(err) => {
                    eprintln!("failed to perform tls handshake: {err:#}");
                    return Err(Box::new(err) as _);
                }
            };

            let mut connection = pin!(hyper.build_connection(tls_stream, service));

            poll_fn(|cx| connection.as_mut().poll_connection(cx)).await
        })
    }

    fn graceful_shutdown(&mut self) {
        ConnectionBuilder::<Io, S>::graceful_shutdown(&mut self.inner);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 3000);

    println!("Starting to serve on https://{addr}");

    // Create a regular axum app.
    let app = Router::new().route("/", get(|| async { "Hello!" }));

    // Create a `TcpListener` using tokio.
    let listener = TcpListener::bind(addr).await.unwrap();

    // Create a connection builder which first drives the TLS handshake and then uses `hyper` to
    // serve the connection.
    let connection_builder = HyperRustls::try_new()?;

    axum::serve::serve(listener, app)
        .with_connection_builder(connection_builder)
        .await
        .unwrap();

    Ok(())
}

// Load public certificate from file.
fn load_certs(filename: &str) -> io::Result<Vec<CertificateDer<'static>>> {
    // Open certificate file.
    let certfile = fs::File::open(filename)
        .map_err(|e| io::Error::other(format!("failed to open {filename}: {e}")))?;
    let mut reader = io::BufReader::new(certfile);

    // Load and return certificate.
    rustls_pemfile::certs(&mut reader).collect()
}

// Load private key from file.
fn load_private_key(filename: &str) -> io::Result<PrivateKeyDer<'static>> {
    // Open keyfile.
    let keyfile = fs::File::open(filename)
        .map_err(|e| io::Error::other(format!("failed to open {filename}: {e}")))?;
    let mut reader = io::BufReader::new(keyfile);

    // Load and return a single private key.
    rustls_pemfile::private_key(&mut reader).map(|key| key.unwrap())
}
