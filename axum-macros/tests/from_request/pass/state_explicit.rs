use axum_macros::FromRequest;
use axum::extract::{FromRef, State};

#[derive(FromRequest)]
#[from_request(state(AppState))]
struct Extractor {
    app_state: State<AppState>,
    one: State<One>,
    two: State<Two>,
}

#[derive(Clone)]
struct AppState {
    one: One,
    two: Two,
}

#[derive(Clone)]
struct One {}

impl FromRef<AppState> for One {
    fn from_ref(input: &AppState) -> Self {
        input.one.clone()
    }
}

#[derive(Clone)]
struct Two {}

impl FromRef<AppState> for Two {
    fn from_ref(input: &AppState) -> Self {
        input.two.clone()
    }
}

fn assert_from_request()
where
    Extractor: axum::extract::FromRequest<AppState, axum::body::Body, Rejection = axum::response::Response>,
{
}

fn main() {}
