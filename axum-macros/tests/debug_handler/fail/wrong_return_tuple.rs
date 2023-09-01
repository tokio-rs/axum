
#[axum::debug_handler]
async fn handler() -> (
    axum::http::StatusCode,
    axum::Json<&'static str>,
    axum::response::AppendHeaders<[( axum::http::HeaderName,&'static str); 1]>,
) {
    panic!()
}

fn main(){}