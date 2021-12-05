//! Run with
//!
//! ```not_rust
//! cargo run -p example-versioning
//! ```

use axum::{
    async_trait,
    extract::{FromRequest, Path, RequestParts},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::{collections::HashMap, net::SocketAddr};

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "example_versioning=debug")
    }
    tracing_subscriber::fmt::init();

    // build our application with some routes
    let app = Router::new().route("/:version/foo", get(handler));

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler(version: Version) {
    println!("received request with version {:?}", version);
}

#[derive(Debug)]
enum Version {
    V1,
    V2,
    V3,
}

#[async_trait]
impl<B> FromRequest<B> for Version
where
    B: Send,
{
    type Rejection = Response;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let params = Path::<HashMap<String, String>>::from_request(req)
            .await
            .map_err(IntoResponse::into_response)?;

        let version = params
            .get("version")
            .ok_or_else(|| (StatusCode::NOT_FOUND, "version param missing").into_response())?;

        match version.as_str() {
            "v1" => Ok(Version::V1),
            "v2" => Ok(Version::V2),
            "v3" => Ok(Version::V3),
            _ => Err((StatusCode::NOT_FOUND, "unknown version").into_response()),
        }
    }
}
