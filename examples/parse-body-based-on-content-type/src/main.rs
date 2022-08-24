//! Provides a RESTful web server managing some Todos.
//!
//! API will be:
//!
//! - `GET /todos`: return a JSON list of Todos.
//! - `POST /todos`: create a new Todo.
//! - `PUT /todos/:id`: update a specific Todo.
//! - `DELETE /todos/:id`: delete a specific Todo.
//!
//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-todos
//! ```

use axum::{
    async_trait,
    extract::FromRequest,
    http::{header::CONTENT_TYPE, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Form, Json, RequestExt, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| {
                "example_parse_body_based_on_content_type=debug,tower_http=debug".into()
            }),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new().route("/", post(handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Serialize, Deserialize)]
struct Payload {
    foo: String,
}

async fn handler(payload: JsonOrForm<Payload>) -> Response {
    match payload {
        JsonOrForm::Json(payload) => Json(payload).into_response(),
        JsonOrForm::Form(payload) => Form(payload).into_response(),
    }
}

enum JsonOrForm<T, K = T> {
    Json(T),
    Form(K),
}

#[async_trait]
impl<S, B, T, K> FromRequest<S, B> for JsonOrForm<T, K>
where
    B: Send + 'static,
    S: Send + Sync,
    Json<T>: FromRequest<(), B>,
    Form<K>: FromRequest<(), B>,
    T: 'static,
    K: 'static,
{
    type Rejection = Response;

    async fn from_request(req: Request<B>, _state: &S) -> Result<Self, Self::Rejection> {
        let content_type = req
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_owned())
            .ok_or_else(|| StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())?;

        if content_type.starts_with("application/json") {
            let Json(payload) = req.extract().await.map_err(IntoResponse::into_response)?;
            return Ok(Self::Json(payload));
        }

        if content_type.starts_with("application/x-www-form-urlencoded") {
            let Form(payload) = req.extract().await.map_err(IntoResponse::into_response)?;
            return Ok(Self::Form(payload));
        }

        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())
    }
}
