use axum::{
    extract::{FromRef, Query, State},
    routing::get,
    Router,
};
use axum_macros::FromRequestParts;
use std::collections::HashMap;

fn main() {
    let _: axum::Router = Router::new()
        .route("/b", get(|_: Extractor| async {}))
        .with_state(AppState::default());
}

#[derive(FromRequestParts)]
#[from_request(state(AppState))]
struct Extractor {
    inner_state: State<InnerState>,
    other: Query<HashMap<String, String>>,
}

#[derive(Default, Clone)]
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
