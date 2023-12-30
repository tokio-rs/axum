#![allow(unused_parens)]

struct NotIntoResponse;

#[axum::debug_handler]
async fn handler() -> (NotIntoResponse) {
    panic!()
}

fn main() {}
