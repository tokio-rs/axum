use axum::{extract::State, routing::get, Router};
use axum_macros::FromRequest;

fn main() {
    let _: axum::Router = Router::new()
        .route("/b", get(|_: AppState| async {}))
        .with_state(AppState::default());
}

// if we're extract "via" `State<AppState>` and not specifying state
// assume `AppState` is the state
#[derive(Clone, Default, FromRequest)]
#[from_request(via(State))]
struct AppState {}
