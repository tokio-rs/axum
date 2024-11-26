use axum::extract::State;
use axum_macros::FromRequest;

#[derive(FromRequest)]
struct Extractor {
    inner_state: State<AppState>,
    also_inner_state: State<AppState>,
}

#[derive(Clone)]
struct AppState {}

fn assert_from_request()
where
    Extractor: axum::extract::FromRequest<AppState, Rejection = axum::response::Response>,
{
}

fn main() {}
