use axum::extract::Json;
use axum_macros::debug_handler;
use serde::Deserialize;

#[derive(Deserialize)]
struct Hello<const N: usize> {
    #[serde(skip)]
    values: Option<[u8; N]>,
}

#[debug_handler]
fn handler<const N: usize>(extract_n: Json<Hello<N>>) -> &'static str {
    "hi!"
}

fn main() {}
