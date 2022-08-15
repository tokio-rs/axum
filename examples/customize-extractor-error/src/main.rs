//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-customize-extractor-error
//! ```

use axum::{
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::IntoResponse,
    routing::post, Json, Router,
};
use axum_extra::extract::WithRejection;
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "example_customize_extractor_error=trace".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // build our application with a route
    let app = Router::new().route("/users", post(handler));

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler(
    // `WithRejection` will extract `Json<User>` from the request. If the
    // extraction fails, a `MyRejection` will be created from `JsonResponse` and
    // returned to the client
    WithRejection(Json(user), _): WithRejection<Json<User>, MyRejection>
) {
    dbg!(&user);
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct User {
    id: i64,
    username: String,
}

// Define your own custom rejection
#[derive(Debug)]
struct MyRejection {
    body: String,
    status: StatusCode,
}

// `IntoResponse` is required for your custom rejection type
impl IntoResponse for MyRejection {
    fn into_response(self) -> axum::response::Response {
        let Self { body, status } = self;
        (
            status,
            // we use `axum::Json` here to generate a JSON response
            // body but you can use whatever response you want
            axum::Json(json!({ "error": body })),
        )
            .into_response()
    }
}

// Implement `From` for any Rejection type you want
impl From<JsonRejection> for MyRejection {
    fn from(rejection: JsonRejection) -> Self {
        // convert the error from `axum::Json` into whatever we want
        let (status, body) = match rejection {
            JsonRejection::JsonDataError(err) => (
                StatusCode::BAD_REQUEST,
                format!("Invalid JSON request: {}", err),
            ),
            JsonRejection::MissingJsonContentType(err) => {
                (StatusCode::BAD_REQUEST, err.to_string())
            }
            err => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Unknown internal error: {}", err),
            ),
        };
        Self { body, status }
    }
}
