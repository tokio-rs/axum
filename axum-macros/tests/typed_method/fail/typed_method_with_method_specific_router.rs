use axum::{routing::MethodFilter, Router};
use axum_extra::routing::{Get, RouterExt, TypedMethod, TypedPath};
use serde::Deserialize;

#[derive(TypedPath, TypedMethod, Deserialize)]
#[typed_path("/users/{id}")]
#[typed_method(MethodFilter::GET)]
struct GetUser {
    id: u32,
}

async fn get_user(_: GetUser) {}

#[derive(TypedPath, Deserialize)]
#[typed_path("/shared/{id}")]
struct SharedPath {
    id: u32,
}

async fn get_shared(_: Get<SharedPath>) {}

fn main() {
    _ = Router::<()>::new().typed_get(get_user);
    _ = Router::<()>::new().typed_put(get_user);
    _ = Router::<()>::new().typed_get(get_shared);
    _ = Router::<()>::new().typed_put(get_shared);
}
