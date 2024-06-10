use axum::{debug_middleware, extract::Request, middleware::Next, response::Response};

#[debug_middleware]
async fn my_middleware(request: Request, next: Next, next2: Next) -> Response {
    let _ = next2;
    next.run(request).await
}

fn main() {}
