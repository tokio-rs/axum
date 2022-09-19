use std::convert::Infallible;
use axum::{
    extract::State,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use axum_macros::FromRequest;

fn main() {
    let _: Router<AppState> =
        Router::with_state(AppState::default()).route("/a", get(|_: Extractor| async {}));
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
