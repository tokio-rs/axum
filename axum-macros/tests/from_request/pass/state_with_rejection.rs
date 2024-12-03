use axum::{
    extract::State,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use axum_macros::FromRequest;
use std::convert::Infallible;

fn main() {
    let _: axum::Router = Router::new()
        .route("/a", get(|_: Extractor| async {}))
        .with_state(AppState::default());
}

#[derive(Clone, Default, FromRequest)]
#[from_request(rejection(MyRejection))]
struct Extractor {
    state: State<AppState>,
}

#[derive(Clone, Default)]
struct AppState {}

struct MyRejection {}

impl From<Infallible> for MyRejection {
    fn from(err: Infallible) -> Self {
        match err {}
    }
}

impl IntoResponse for MyRejection {
    fn into_response(self) -> Response {
        ().into_response()
    }
}
