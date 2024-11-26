use axum::{debug_middleware, extract::Request, middleware::Next, response::Response};

#[debug_middleware]
async fn my_middleware(request: Request, next: Next) -> Response {
    next.run(request).await
}

fn main() {}
