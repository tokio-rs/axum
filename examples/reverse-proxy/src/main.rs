//! Reverse proxy listening in "localhost:4000" will proxy all `GET` requests to "localhost:3000" except for path /https is example.com
//! endpoint.
//!
//! On unix like OS: make sure `ca-certificates` is installed.
//!
//! Run with
//!
//! ```not_rust
//! cargo run -p example-reverse-proxy
//! ```

use axum::http::{header::HOST, StatusCode};
use axum::{
    body::Body,
    extract::{Request, State},
    http::uri::Uri,
    response::{IntoResponse, Response},
    routing, Router,
};
use hyper_tls::HttpsConnector;
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

type Client = hyper_util::client::legacy::Client<HttpsConnector<HttpConnector>, Body>;

#[tokio::main]
async fn main() {
    tokio::spawn(server());

    let client: Client =
        hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
            .build(HttpsConnector::new());

    let app = Router::new()
        .fallback(routing::get(handler))
        .with_state(client);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:4000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn handler(State(client): State<Client>, mut req: Request) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    let path_query = req
        .uri()
        .path_and_query()
        .map(|v| v.as_str())
        .unwrap_or(path);

    let mut uri = format!("http://127.0.0.1:3000{}", path_query);
    if path == "/https" {
        uri = String::from("https://example.com");
    }

    *req.uri_mut() = Uri::try_from(uri).unwrap();

    //? Remove incorrect header host, hyper will add automatically for you.
    req.headers_mut().remove(HOST).unwrap();

    Ok(client
        .request(req)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .into_response())
}

async fn server() {
    let app = Router::new().fallback(routing::get(|| async { "Hello, world!" }));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
