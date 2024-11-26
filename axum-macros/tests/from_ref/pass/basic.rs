use axum::{
    extract::{FromRef, State},
    routing::get,
    Router,
};

// This will implement `FromRef` for each field in the struct.
#[derive(Clone, FromRef)]
struct AppState {
    auth_token: String,
}

// So those types can be extracted via `State`
async fn handler(_: State<String>) {}

fn main() {
    let state = AppState {
        auth_token: Default::default(),
    };

    let _: axum::Router = Router::new().route("/", get(handler)).with_state(state);
}
