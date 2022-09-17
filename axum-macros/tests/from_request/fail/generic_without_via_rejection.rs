use axum::{body::Body, routing::get, Router};
use axum_macros::FromRequest;

#[derive(FromRequest, Clone)]
#[from_request(rejection(Foo))]
struct Extractor<T>(T);

async fn foo(_: Extractor<()>) {}

fn main() {
    Router::<(), Body>::new().route("/", get(foo));
}
