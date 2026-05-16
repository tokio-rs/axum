use axum::{routing::get, Router};
use axum_macros::FromRequest;

struct Payload;

#[derive(FromRequest)]
struct Args {
    payload: Option<Payload>,
}

async fn handler(_: Args) {}

fn main() {
    let _: Router = Router::new().route("/", get(handler));
}
