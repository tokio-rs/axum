//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-hello-world
//! ```

use axum::{handler::Handler, response::Html, routing::get, Router};
use std::net::SocketAddr;
use tower::layer::util::Identity;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route(
            "/",
            get(handler.layer(Identity::new()))
                .layer(Identity::new())
                .route_layer(Identity::new()),
        )
        .layer(Identity::new())
        .route_layer(Identity::new());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}
