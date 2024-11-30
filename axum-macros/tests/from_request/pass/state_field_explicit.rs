use axum::{
    extract::{FromRef, State},
    routing::get,
    Router,
};
use axum_macros::FromRequest;

fn main() {
    let _: axum::Router = Router::new()
        .route("/", get(|_: Extractor| async {}))
        .with_state(AppState::default());
}

#[derive(FromRequest)]
#[from_request(state(AppState))]
struct Extractor {
    #[from_request(via(State))]
    state: AppState,
    #[from_request(via(State))]
    inner: InnerState,
}

#[derive(Clone, Default)]
struct AppState {
    inner: InnerState,
}

#[derive(Clone, Default)]
struct InnerState {}

impl FromRef<AppState> for InnerState {
    fn from_ref(input: &AppState) -> Self {
        input.inner.clone()
    }
}
