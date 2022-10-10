use axum_macros::FromRef;
use axum::{Router, routing::get, extract::State};

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

    let _: Router<AppState> = Router::with_state(state).route("/", get(handler));
}
