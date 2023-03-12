use axum::{routing::get, Extension, Router};
use axum_macros::FromRequestParts;

#[derive(FromRequestParts, Clone)]
#[from_request(via(Extension))]
enum Extractor {}

async fn foo(_: Extractor) {}

fn main() {
    _ = Router::<()>::new().route("/", get(foo));
}
