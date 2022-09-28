//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-hello-world
//! ```

use axum::{extract::State, routing::get, Router};
use axum_macros::FromRef;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let app = Router::with_state(AppState::default()).route("/", get(|_: State<String>| async {}));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(FromRef, Default)]
struct AppState {
    token: String,
    #[from_ref(skip)]
    skip: NotClone,
}

#[derive(Default)]
struct NotClone {}
