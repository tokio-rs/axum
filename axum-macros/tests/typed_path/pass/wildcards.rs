use axum_extra::routing::{RouterExt, TypedPath};
use serde::Deserialize;

#[derive(TypedPath, Deserialize)]
#[typed_path("/*rest")]
struct MyPath {
    rest: String,
}

fn main() {
    axum::Router::<()>::new().typed_get(|_: MyPath| async {});
}
