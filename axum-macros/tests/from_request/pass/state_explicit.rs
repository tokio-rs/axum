use axum::{
    extract::{FromRef, State},
    routing::get,
    Router,
};
use axum_macros::FromRequest;

fn main() {
    let _: axum::Router = Router::new()
        .route("/b", get(|_: Extractor| async {}))
        .with_state(AppState::default());
}

#[derive(FromRequest)]
#[from_request(state(AppState))]
struct Extractor {
    app_state: State<AppState>,
    one: State<One>,
    two: State<Two>,
    other_extractor: String,
}

#[derive(Clone, Default)]
struct AppState {
    one: One,
    two: Two,
}

#[derive(Clone, Default)]
struct One {}

impl FromRef<AppState> for One {
    fn from_ref(input: &AppState) -> Self {
        input.one.clone()
    }
}

#[derive(Clone, Default)]
struct Two {}

impl FromRef<AppState> for Two {
    fn from_ref(input: &AppState) -> Self {
        input.two.clone()
    }
}
