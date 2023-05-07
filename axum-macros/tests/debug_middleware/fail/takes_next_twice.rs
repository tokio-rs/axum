use axum::{
    extract::Request,
    response::Response,
    middleware::Next,
    debug_middleware,
};

#[debug_middleware]
async fn my_middleware(request: Request, next: Next, next2: Next) -> Response {
    next.run(request).await
}

fn main() {}
