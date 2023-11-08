//! Run with
//!
//! ```not_rust
//! cargo shuttle run --working-directory examples/hosting-on-shuttle
//! ```

use axum::{routing::get, Router};

#[shuttle_runtime::main]
async fn main() -> shuttle_axum::ShuttleAxum {
    // build our application with a route
    let router = Router::new().route("/", get(hello_world));

    // start the server
    tracing::info!("starting the server");
    Ok(router.into())
}

async fn hello_world() -> &'static str {
    "Hello, world!"
}
