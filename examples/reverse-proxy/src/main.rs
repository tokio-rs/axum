//! Reverse proxy listening in "localhost:4000" will proxy all requests to "localhost:3000"
//! endpoint.
//!
//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-reverse-proxy
//! ```

use axum::{
    extract::State,
    http::{uri::Uri, Request, Response},
    routing::get,
    Router,
};
use hyper::{client::HttpConnector, Body};
use std::net::SocketAddr;

type Client = hyper::client::Client<HttpConnector, Body>;

#[tokio::main]
async fn main() {
    tokio::spawn(server());

    let client = Client::new();

    let app = Router::new().route("/", get(handler)).with_state(client);

    let addr = SocketAddr::from(([127, 0, 0, 1], 4000));
    println!("reverse proxy listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler(State(client): State<Client>, mut req: Request<Body>) -> Response<Body> {
    let path = req.uri().path();
    let path_query = req
        .uri()
        .path_and_query()
        .map(|v| v.as_str())
        .unwrap_or(path);

    let uri = format!("http://127.0.0.1:3000{}", path_query);

    *req.uri_mut() = Uri::try_from(uri).unwrap();

    client.request(req).await.unwrap()
}

async fn server() {
    let app = Router::new().route("/", get(|| async { "Hello, world!" }));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("server listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
