use axum_macros::debug_handler;
use axum::{extract::State, http::Request};

#[debug_handler(state = AppState)]
async fn handler(_: State<AppState>, _: Request<axum::body::Body>) {}

#[derive(Clone)]
struct AppState;

fn main() {}
