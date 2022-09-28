//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-routes-and-handlers-close-together
//! ```

use axum::{
    routing::{get, post, MethodRouter},
    Router,
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .merge(root())
        .merge(get_foo())
        .merge(post_foo());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn root() -> Router {
    async fn handler() -> &'static str {
        "Hello, World!"
    }

    route("/", get(handler))
}

fn get_foo() -> Router {
    async fn handler() -> &'static str {
        "Hi from `GET /foo`"
    }

    route("/foo", get(handler))
}

fn post_foo() -> Router {
    async fn handler() -> &'static str {
        "Hi from `POST /foo`"
    }

    route("/foo", post(handler))
}

fn route(path: &str, method_router: MethodRouter<()>) -> Router {
    Router::new().route(path, method_router)
}
