use axum_macros::debug_handler;
use axum::{extract::State, extract::Request};

#[debug_handler(state = AppState)]
async fn handler(_: State<AppState>, _: Request) {}

#[derive(Clone)]
struct AppState;

fn main() {}
