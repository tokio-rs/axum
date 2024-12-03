use axum::{extract::State, routing::get, Router};
use axum_macros::FromRequest;

fn main() {
    let _: axum::Router = Router::new()
        .route("/", get(|_: Extractor| async {}))
        .with_state(AppState::default());
}

#[derive(FromRequest)]
struct Extractor {
    #[from_request(via(State))]
    state: AppState,
}

#[derive(Clone, Default)]
struct AppState {}
