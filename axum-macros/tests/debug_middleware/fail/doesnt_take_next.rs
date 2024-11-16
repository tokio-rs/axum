use axum::{
    debug_middleware,
    extract::Request,
    response::{IntoResponse, Response},
};

#[debug_middleware]
async fn my_middleware(request: Request) -> Response {
    let _ = request;
    ().into_response()
}

fn main() {}
