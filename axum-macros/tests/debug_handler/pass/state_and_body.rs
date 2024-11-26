use axum::{extract::Request, extract::State};
use axum_macros::debug_handler;

#[debug_handler(state = AppState)]
async fn handler(_: State<AppState>, _: Request) {}

#[derive(Clone)]
struct AppState;

fn main() {}
