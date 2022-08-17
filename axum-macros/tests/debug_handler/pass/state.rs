use axum_macros::debug_handler;
use axum::extract::State;

#[debug_handler(state = AppState)]
async fn handler(_: State<AppState>) {}

#[derive(Clone)]
struct AppState;

fn main() {}
