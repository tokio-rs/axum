use axum::{
    extract::{FromRef, State},
    routing::get,
    Router,
};
use axum_macros::FromRequestParts;

fn main() {
    let _: axum::Router = Router::new()
        .route("/a", get(|_: AppState, _: InnerState, _: String| async {}))
        .route("/b", get(|_: AppState, _: String| async {}))
        .route("/c", get(|_: InnerState, _: String| async {}))
        .with_state(AppState::default());
}

#[derive(Clone, Default, FromRequestParts)]
#[from_request(via(State))]
struct AppState {
    inner: InnerState,
}

#[derive(Clone, Default, FromRequestParts)]
#[from_request(via(State), state(AppState))]
struct InnerState {}

impl FromRef<AppState> for InnerState {
    fn from_ref(input: &AppState) -> Self {
        input.inner.clone()
    }
}
