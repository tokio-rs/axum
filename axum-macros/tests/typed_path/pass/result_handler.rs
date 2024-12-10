use axum::{extract::rejection::PathRejection, http::StatusCode};
use axum_extra::routing::{RouterExt, TypedPath};
use serde::Deserialize;

#[derive(TypedPath, Deserialize)]
#[typed_path("/users/{id}")]
struct UsersShow {
    id: String,
}

async fn result_handler(_: Result<UsersShow, PathRejection>) {}

#[derive(TypedPath, Deserialize)]
#[typed_path("/users")]
struct UsersIndex;

async fn result_handler_unit_struct(_: Result<UsersIndex, StatusCode>) {}

fn main() {
    _ = axum::Router::<()>::new()
        .typed_post(result_handler)
        .typed_post(result_handler_unit_struct);
}
