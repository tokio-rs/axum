use axum::{
    extract::{FromRef, State},
    routing::get,
    Router,
};
use axum_macros::FromRequestParts;

fn main() {
    let _: axum::Router = Router::new()
        .route("/a", get(|_: AppState| async {}))
        .route("/b", get(|_: InnerState| async {}))
        .route("/c", get(|_: AppState, _: InnerState| async {}))
        .with_state(AppState::default());
}

#[derive(Clone, FromRequestParts)]
#[from_request(via(State))]
enum AppState {
    One,
}

impl Default for AppState {
    fn default() -> AppState {
        Self::One
    }
}

#[derive(FromRequestParts)]
#[from_request(via(State), state(AppState))]
enum InnerState {}

impl FromRef<AppState> for InnerState {
    fn from_ref(_: &AppState) -> Self {
        todo!("🤷")
    }
}
