#[axum::debug_handler]
async fn handler() -> (
    axum::http::request::Parts, // this should be response parts, not request parts
    axum::http::StatusCode,
) {
    panic!()
}

fn main() {}
