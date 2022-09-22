use axum_macros::FromRequest;
use axum::extract::State;

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
    Extractor: axum::extract::FromRequest<AppState, axum::body::Body, Rejection = axum::response::Response>,
{
}

fn main() {}
