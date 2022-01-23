//! Run with
//!
//! ```not_rust
//! cargo run -p example-hello-world
//! ```

use axum::{response::Html, routing::get, Router};
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

async fn handler(_: Unit, _: Tuple, _: Named) -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

#[derive(axum_macros::FromRequest)]
struct Unit;

#[derive(axum_macros::FromRequest)]
struct Tuple(String);

#[derive(axum_macros::FromRequest)]
struct Named {
    body: String,
}
