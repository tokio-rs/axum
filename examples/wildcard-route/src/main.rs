//! Run with
//!
//! ```not_rust
//! cargo run -p example-wildcard-route
//! ```

use axum::{http::Uri, response::Html, routing::get, Router};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(handler))
        .route("/*rest", get(handler));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn handler(uri: Uri) -> Html<String> {
    Html(format!("<h1>uri={uri}</h1>"))
}
