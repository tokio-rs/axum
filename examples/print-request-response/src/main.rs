//! Run with
//!
//! ```not_rust
//! cargo run -p example-print-request-response
//! ```

use axum::{
    body::{Body, BoxBody, Bytes},
    error_handling::HandleErrorLayer,
    http::{Request, Response, StatusCode},
    routing::post,
    Router,
};
use std::net::SocketAddr;
use tower::{filter::AsyncFilterLayer, util::AndThenLayer, BoxError, ServiceBuilder};

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var(
            "RUST_LOG",
            "example_print_request_response=debug,tower_http=debug",
        )
    }
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/", post(|| async move { "Hello from `POST /`" }))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|error| async move {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled internal error: {}", error),
                    )
                }))
                .layer(AndThenLayer::new(map_response))
                .layer(AsyncFilterLayer::new(map_request)),
        );

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn map_request(req: Request<Body>) -> Result<Request<Body>, BoxError> {
    let (parts, body) = req.into_parts();
    let bytes = buffer_and_print("request", body).await?;
    let req = Request::from_parts(parts, Body::from(bytes));
    Ok(req)
}

async fn map_response(res: Response<BoxBody>) -> Result<Response<Body>, BoxError> {
    let (parts, body) = res.into_parts();
    let bytes = buffer_and_print("response", body).await?;
    let res = Response::from_parts(parts, Body::from(bytes));
    Ok(res)
}

async fn buffer_and_print<B>(direction: &str, body: B) -> Result<Bytes, BoxError>
where
    B: axum::body::HttpBody<Data = Bytes>,
    B::Error: Into<BoxError>,
{
    let bytes = hyper::body::to_bytes(body).await.map_err(Into::into)?;
    if let Ok(body) = std::str::from_utf8(&bytes) {
        tracing::debug!("{} body = {:?}", direction, body);
    }
    Ok(bytes)
}
