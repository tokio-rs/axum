use axum::extract::State;
use axum_macros::FromRequest;

#[derive(FromRequest)]
struct Extractor {
    inner_state: State<AppState>,
    other_state: State<OtherState>,
}

#[derive(Clone)]
struct AppState {}

#[derive(Clone)]
struct OtherState {}

fn assert_from_request()
where
    Extractor: axum::extract::FromRequest<AppState, Rejection = axum::response::Response>,
{
}

fn main() {}
