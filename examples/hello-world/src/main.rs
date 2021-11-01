//! Run with
//!
//! ```not_rust
//! cargo run -p example-hello-world
//! ```

#[tokio::main]
async fn main() {
    use axum::async_trait;
    use axum::http::StatusCode;
    use axum::{
        extract::{extractor_middleware, FromRequest, RequestParts},
        routing::{get, post},
        Router,
    };

    // An extractor that performs authorization.
    struct RequireAuth;

    #[async_trait]
    impl<B> FromRequest<B> for RequireAuth
    where
        B: Send,
    {
        type Rejection = StatusCode;

        async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
            let auth_header = req
                .headers()
                .and_then(|headers| headers.get(axum::http::header::AUTHORIZATION))
                .and_then(|value| value.to_str().ok());

            if let Some(value) = auth_header {
                if value == "secret" {
                    return Ok(Self);
                }
            }

            Err(StatusCode::UNAUTHORIZED)
        }
    }

    async fn handler() {
        // If we get here the request has been authorized
    }

    async fn other_handler() {
        // If we get here the request has been authorized
    }

    let app = Router::new()
        .route("/", get(handler))
        .route("/foo", post(other_handler))
        // The extractor will run before all routes
        .layer(extractor_middleware::<RequireAuth>());

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
