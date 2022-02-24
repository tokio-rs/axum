use axum_extra::routing::TypedPath;
use axum::http::Uri;
use serde::Deserialize;
use std::convert::TryInto;

#[derive(TypedPath, Deserialize)]
#[typed_path("/:id")]
struct Named {
    id: u32,
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/:id")]
struct Unnamed(u32);

#[derive(TypedPath, Deserialize)]
#[typed_path("/")]
struct Unit;

fn main() {
    let _: Uri = Named { id: 1 }.try_into().unwrap();
    let _: Uri = Unnamed(1).try_into().unwrap();
    let _: Uri = Unit.try_into().unwrap();
}
