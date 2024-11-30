#![allow(unused_parens)]

#[axum::debug_handler]
async fn named_type() -> (
    axum::http::StatusCode,
    axum::Json<&'static str>,
    axum::response::AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
) {
    panic!()
}

struct CustomIntoResponse {}
impl axum::response::IntoResponse for CustomIntoResponse {
    fn into_response(self) -> axum::response::Response {
        todo!()
    }
}
#[axum::debug_handler]
async fn custom_type() -> (
    axum::http::StatusCode,
    CustomIntoResponse,
    axum::response::AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
) {
    panic!()
}

fn main() {}
