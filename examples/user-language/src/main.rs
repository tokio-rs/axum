//! Run with
//!
//! ```not_rust
//! cargo run -p example-user-language
//! ```

use axum::{response::Html, routing::get, Router};
use axum_extra::extract::UserLanguage;

#[tokio::main]
async fn main() {
    // build our application with a route
    let app = Router::new()
        .route("/", get(handler))
        .route("/:lang", get(handler));

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn handler(lang: UserLanguage) -> Html<&'static str> {
    println!(
        "User prefers content in the following languages (in order): {:?}",
        lang.preferred_languages()
    );

    match lang.preferred_language() {
        "de" => Html("<h1>Hallo, Welt!</h1>"),
        "es" => Html("<h1>Hola, Mundo!</h1>"),
        "fr" => Html("<h1>Bonjour, le monde!</h1>"),
        _ => Html("<h1>Hello, World!</h1>"),
    }
}
