//! Run with
//!
//! ```not_rust
//! cargo run -p example-form
//! ```

use axum::{extract::Form, handler::get, response::Html, Router};
use serde::Deserialize;
use std::net::SocketAddr;
use validator::Validate;

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "example_form=debug")
    }
    tracing_subscriber::fmt::init();

    // build our application with some routes
    let app = Router::new().route("/", get(show_form).post(accept_form));

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn show_form(errors: Option<String>) -> Html<String> {
    Html(format!(
        r#"
        <!doctype html>
        <html>
            <head></head>
            <body>
                <!-- In a more realistic application we would use a templating language (see templates example) -->
                <pre>{}</pre>
                <form action="/" method="post">
                    <label for="name">
                        Enter your name:
                        <input type="text" name="name">
                    </label>

                    <label>
                        Enter your email:
                        <input type="text" name="email">
                    </label>

                    <input type="submit" value="Subscribe!">
                </form>
            </body>
        </html>
        "#,
        errors.unwrap_or_default()
    ))
}

#[derive(Deserialize, Debug, Validate)]
struct Input {
    #[validate(length(min = 2))]
    name: String,
    #[validate(email)]
    email: String,
}

async fn accept_form(Form(input): Form<Input>) -> Html<String> {
    dbg!(&input);
    match input.validate() {
        Ok(_) => String::from("Form submitted successfully!").into(),
        // Here the validation errors are presented as-is. In a more realistic
        // application we would customize / process the validation errors to be able to
        // show to the user in a way that they could understand it
        Err(e) => show_form(Some(format!("Form error: {}", e))).await,
    }
}
