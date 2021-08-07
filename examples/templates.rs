//! Run with
//!
//! ```not_rust
//! cargo run --example templates
//! ```

use askama::Template;
use axum::{prelude::*, response::IntoResponse};
use http::{Response, StatusCode};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "templates=debug")
    }
    tracing_subscriber::fmt::init();

    // build our application with some routes
    let app = route("/greet/:name", get(greet));

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn greet(extract::Path(name): extract::Path<String>) -> impl IntoResponse {
    let template = HelloTemplate { name };
    HtmlTemplate(template)
}

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate {
    name: String,
}

struct HtmlTemplate<T>(T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> http::Response<Body> {
        match self.0.render() {
            Ok(html) => response::Html(html).into_response(),
            Err(err) => Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!(
                    "Failed to render template. Error: {}",
                    err
                )))
                .unwrap(),
        }
    }
}
