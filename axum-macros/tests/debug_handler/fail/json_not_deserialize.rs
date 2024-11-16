use axum::Json;
use axum_macros::debug_handler;

struct Struct {}

#[debug_handler]
async fn handler(_foo: Json<Struct>) {}

fn main() {}
