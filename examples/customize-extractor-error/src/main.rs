//! Run with
//!
//! ```not_rust
//! cargo run -p example-customize-extractor-error
//! ```

use axum::{
    async_trait,
    extract::{rejection::JsonRejection, FromRequest, RequestParts},
    http::StatusCode,
    routing::post,
    BoxError, Router,
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};
use std::{borrow::Cow, net::SocketAddr};

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "example_customize_extractor_error=debug")
    }
    tracing_subscriber::fmt::init();

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

async fn handler(Json(user): Json<User>) {
    dbg!(&user);
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct User {
    id: i64,
    username: String,
}

// We define our own `Json` extractor that customizes the error from `axum::Json`
struct Json<T>(T);

#[async_trait]
impl<B, T> FromRequest<B> for Json<T>
where
    // these trait bounds are copied from `impl FromRequest for axum::Json`
    T: DeserializeOwned,
    B: axum::body::HttpBody + Send,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    type Rejection = (StatusCode, axum::Json<Value>);

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req).await {
            Ok(value) => Ok(Self(value.0)),
            Err(rejection) => {
                // convert the error from `axum::Json` into whatever we want
                let (status, body): (_, Cow<'_, str>) = match rejection {
                    JsonRejection::InvalidJsonBody(err) => (
                        StatusCode::BAD_REQUEST,
                        format!("Invalid JSON request: {}", err).into(),
                    ),
                    JsonRejection::MissingJsonContentType(err) => {
                        (StatusCode::BAD_REQUEST, err.to_string().into())
                    }
                    JsonRejection::HeadersAlreadyExtracted(err) => {
                        (StatusCode::INTERNAL_SERVER_ERROR, err.to_string().into())
                    }
                    err => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unknown internal error: {}", err).into(),
                    ),
                };

                Err((
                    status,
                    // we use `axum::Json` here to generate a JSON response
                    // body but you can use whatever response you want
                    axum::Json(json!({ "error": body })),
                ))
            }
        }
    }
}
