use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

use tokio_rustls::rustls::{
    internal::pemfile::certs, internal::pemfile::pkcs8_private_keys, NoClientAuth, ServerConfig,
};

use tokio::net::TcpListener;

use tokio_rustls::TlsAcceptor;

use hyper::server::conn::Http;
use hyper::{Body, Response};

use axum::handler::get;
use axum::route;

#[tokio::main]
async fn main() {
    let rustls_config =
        rustls_server_config("self_signed_certs/key.pem", "self_signed_certs/cert.pem");

    let acceptor = TlsAcceptor::from(rustls_config);
    let listener = TcpListener::bind("127.0.0.1:3443").await.unwrap();

    let app = route(
        "/",
        get(|| async { Response::new(Body::from("Hello, world!")) }),
    );

    loop {
        let (stream, _addr) = listener.accept().await.unwrap();
        let acceptor = acceptor.clone();

        let app = app.clone();

        tokio::spawn(async move {
            if let Ok(stream) = acceptor.accept(stream).await {
                let fut = Http::new().serve_connection(stream, app);

                match fut.await {
                    Ok(()) => (),
                    Err(_) => (),
                }
            }
        });
    }
}

fn rustls_server_config(key: &str, cert: &str) -> Arc<ServerConfig> {
    let mut config = ServerConfig::new(NoClientAuth::new());

    let mut key_reader = BufReader::new(File::open(key).unwrap());
    let mut cert_reader = BufReader::new(File::open(cert).unwrap());

    let key = pkcs8_private_keys(&mut key_reader).unwrap().remove(0);
    let certs = certs(&mut cert_reader).unwrap();

    config.set_single_cert(certs, key).unwrap();

    config.set_protocols(&[b"h2".to_vec(), b"http/1.1".to_vec()]);

    Arc::new(config)
}
