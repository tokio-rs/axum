//! Reverse proxy listening in "localhost:4000" will proxy all `GET` requests to "localhost:3000"
//! except for path /https is example.com endpoint.
//!
//! On unix like OS: make sure `ca-certificates` is installed.
//!
//! Run with
//!
//! ```not_rust
//! cargo run -p example-reverse-proxy
//! ```

use axum::extract::ConnectInfo;
use axum::http::header::FORWARDED;
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
use std::net::SocketAddr;

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
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

async fn handler(
    State(client): State<Client>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut req: Request,
) -> Result<Response, StatusCode> {
    let uri = req.uri();

    let ip = addr.ip().to_string();
    let host = uri
        .authority()
        .map(|a| a.as_str())
        .unwrap_or("127.0.0.1:4000")
        .to_string();
    let proto = uri.scheme_str().unwrap_or("http").to_string();

    let path = uri.path();
    let path_query = uri.path_and_query().map(|v| v.as_str()).unwrap_or(path);

    let mut uri = format!("http://127.0.0.1:3000{}", path_query);
    if path == "/https" {
        uri = String::from("https://example.com");
    }

    *req.uri_mut() = Uri::try_from(uri).unwrap();

    // Remove incorrect header host, hyper will add automatically for you.
    req.headers_mut().remove(HOST);

    // Add some informative header (de-facto)
    req.headers_mut()
        .insert("X-Forwarded-For", ip.parse().unwrap());
    req.headers_mut()
        .insert("X-Forwarded-Host", host.parse().unwrap());
    req.headers_mut()
        .insert("X-Forwarded-Proto", proto.parse().unwrap());

    // a standardized
    req.headers_mut().insert(
        FORWARDED,
        format!("for={ip};host={host};proto={proto};")
            .parse()
            .unwrap(),
    );

    Ok(client
        .request(req)
        .await
        .map_err(|err| {
            eprintln!("{:?}", err);
            StatusCode::BAD_REQUEST
        })?
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
