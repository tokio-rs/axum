//! Run with
//!
//! ```not_rust
//! cargo run -p example-user-language
//! ```

use axum::{response::Html, routing::get, Extension, Router};
use axum_extra::extract::user_lang::{PathSource, QuerySource, UserLanguage};

#[tokio::main]
async fn main() {
    // build our application with some routes
    let app = Router::new()
        .route("/", get(handler))
        .route("/:lang", get(handler))
        // Add configuration for the `UserLanguage` extractor.
        // This step is optional, if omitted the default
        // configuration will be used.
        .layer(Extension(
            UserLanguage::config()
                // read the language from the `lang` query parameter
                .add_source(QuerySource::new("lang"))
                // read the language from the `:lang` segment of the path
                .add_source(PathSource::new("lang"))
                .build(),
        ));

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
