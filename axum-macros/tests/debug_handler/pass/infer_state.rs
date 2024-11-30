use axum::extract::State;
use axum_macros::debug_handler;

#[debug_handler]
async fn handler(_: State<AppState>) {}

#[debug_handler]
async fn handler_2(_: axum::extract::State<AppState>) {}

#[debug_handler]
async fn handler_3(_: axum::extract::State<AppState>, _: axum::extract::State<AppState>) {}

#[debug_handler]
async fn handler_4(_: State<AppState>, _: State<AppState>) {}

#[debug_handler]
async fn handler_5(_: axum::extract::State<AppState>, _: State<AppState>) {}

#[derive(Clone)]
struct AppState;

fn main() {}
