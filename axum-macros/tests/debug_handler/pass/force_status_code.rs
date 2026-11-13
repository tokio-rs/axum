use axum::{
    debug_handler,
    http::{HeaderMap, StatusCode},
    response::ForceStatusCode,
    routing::get,
    Router,
};

#[debug_handler]
async fn with_body() -> (ForceStatusCode, &'static str) {
    (ForceStatusCode(StatusCode::CREATED), "body")
}

#[debug_handler]
async fn with_headers() -> (axum::response::ForceStatusCode, HeaderMap, &'static str) {
    (
        ForceStatusCode(StatusCode::ACCEPTED),
        HeaderMap::new(),
        "body",
    )
}

fn main() {
    let _: Router = Router::new()
        .route("/body", get(with_body))
        .route("/headers", get(with_headers));
}
