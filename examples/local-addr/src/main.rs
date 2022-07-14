//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p local-addr
//! ```

// Obtain server's IP:port first from the request's pseudo-header
// authority field, in case of HTTP/2, then from the header's host 
// field, in case of HTTP/1.
//
// Built off the "hello-world" example code.

use axum::{
    http::{header, uri::Authority, HeaderMap, HeaderValue, Uri},
    response::Html,
    routing::get,
    Router,
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // build our application with a route
    let app = Router::new().route("/", get(handler));

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler(headers: HeaderMap<HeaderValue>, uri: Uri) -> Html<String> {
    let host =
        local_addr(uri.authority(), headers.get(header::HOST)).unwrap_or("[local address None]");
    Html(format!("<h1>Greetings from {host}!</h1>"))
}

pub fn local_addr<'l>(
    auth: Option<&'l Authority>,
    host: Option<&'l HeaderValue>,
) -> Option<&'l str> {
    if let Some(auth) = auth {
        Some(auth.as_str())               // HTTP/2
    } else if let Some(host) = host {
        Some(host.to_str().ok().unwrap()) // HTTP/1
    } else {
        None
    }
}
