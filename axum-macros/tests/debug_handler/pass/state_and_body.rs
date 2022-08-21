use axum_macros::debug_handler;
use axum::{body::BoxBody, extract::State, http::Request};

#[debug_handler(state = AppState, body = BoxBody)]
async fn handler(_: State<AppState>, _: Request<BoxBody>) {}

#[derive(Clone)]
struct AppState;

fn main() {}
