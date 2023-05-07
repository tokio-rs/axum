use axum::{
    extract::Request,
    response::{Response, IntoResponse},
    debug_middleware,
};

#[debug_middleware]
async fn my_middleware(request: Request) -> Response {
    ().into_response()
}

fn main() {}
