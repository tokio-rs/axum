use axum::extract::State;
use axum_macros::FromRequestParts;

#[derive(FromRequestParts)]
struct Extractor {
    inner_state: State<AppState>,
}

#[derive(Clone)]
struct AppState {}

fn assert_from_request()
where
    Extractor: axum::extract::FromRequestParts<AppState, Rejection = axum::response::Response>,
{
}

fn main() {}
