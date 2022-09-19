use axum_macros::FromRequestParts;
use axum::{
    extract::{FromRef, State, Query},
    Router,
    routing::get,
};
use std::collections::HashMap;

fn main() {
    let _: Router<AppState> = Router::with_state(AppState::default())
        .route("/b", get(|_: Extractor| async {}));
}

#[derive(FromRequestParts)]
#[from_request(state(AppState))]
struct Extractor {
    inner_state: State<InnerState>,
    other: Query<HashMap<String, String>>,
}

#[derive(Default)]
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
