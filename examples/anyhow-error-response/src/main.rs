//! Run with
//!
//! ```not_rust
//! cargo run -p example-anyhow-error-response
//! ```

use axum::{
    http::StatusCode,
    response::IntoResultResponse,
    routing::get,
    Router, ResultExt,
};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(handler));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn handler() -> impl IntoResultResponse {
    try_thing_anyhow()?; // by default this will return a StatusCode::INTERNAL_SERVER_ERROR (500) error
    try_thing_anyhow().err_with_status(StatusCode::BAD_REQUEST)?; // Using the `ResultExt` trait to return a StatusCode::BAD_REQUEST (400) error

    try_thing_stderror()?; // Standard errors also work
    Ok(())
}

fn try_thing_anyhow() -> Result<(), anyhow::Error> {
    anyhow::bail!("it failed!");
}

fn try_thing_stderror() -> Result<(), impl std::error::Error> {
    Err(std::fmt::Error::default())
}
