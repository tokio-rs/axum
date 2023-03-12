use axum_macros::FromRequest;
use axum::extract::State;

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
