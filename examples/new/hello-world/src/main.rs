//! Run with
//!
//! ```not_rust
//! cargo run -p hello-world
//! ```

use axum::prelude::*;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // build our application with a route
    let app = route("/foo", get(handler));

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler() -> &'static str {
    "<h1>Hello, World!</h1>"
}
