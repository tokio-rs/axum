//! Run with
//!
//! ```not_rust
//! cargo run -p example-validator
//!
//! curl '127.0.0.1:3000?name='
//! -> Input validation error: [name: Can not be empty]
//!
//! curl '127.0.0.1:3000?name=LT'
//! -> <h1>Hello, LT!</h1>
//! ```

use axum::{
    body::{Bytes, Full},
    extract::Form,
    handler::get,
    http::{Response, StatusCode},
    response::{Html, IntoResponse},
    Router,
};
use serde::Deserialize;
use std::{convert::Infallible, net::SocketAddr};
use thiserror::Error;
use validator::Validate;

#[tokio::main]
async fn main() {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "example_validator=debug")
    }
    tracing_subscriber::fmt::init();
    // build our application with a route
    let app = Router::new().route("/", get(handler));

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Debug, Deserialize, Validate)]
pub struct NameInput {
    #[validate(length(min = 1, message = "Can not be empty"))]
    pub name: String,
}

async fn handler(Form(input): Form<NameInput>) -> Result<Html<String>, ServerError> {
    input.validate()?;

    Ok(Html(format!("<h1>Hello, {}!</h1>", input.name)))
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error(transparent)]
    ValidationError(#[from] validator::ValidationErrors),

    #[error("Internal server error")]
    InternalServerError,
}

impl IntoResponse for ServerError {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        match self {
            ServerError::ValidationError(_) => {
                let message = format!("Input validation error: [{}]", self).replace("\n", ", ");
                (StatusCode::BAD_REQUEST, message)
            }
            error => (
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
            ),
        }
        .into_response()
    }
}
