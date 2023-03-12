use axum::{routing::get, Router};
use axum_macros::FromRequest;

#[derive(FromRequest, Clone)]
struct Extractor<T>(T);

async fn foo(_: Extractor<()>) {}

fn main() {
    _ = Router::<()>::new().route("/", get(foo));
}
