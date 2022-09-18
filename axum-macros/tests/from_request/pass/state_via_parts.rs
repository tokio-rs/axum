use axum::{
    extract::{FromRef, State},
    routing::get,
    Router,
};
use axum_macros::FromRequestParts;

#[tokio::main]
async fn main() {
    let _: Router<AppState> = Router::with_state(AppState::default())
        .route("/a", get(|_: AppState, _: InnerState| async {}))
        .route("/b", get(|_: AppState| async {}))
        .route("/c", get(|_: InnerState| async {}));
}

#[derive(Clone, Default, FromRequestParts)]
#[from_request(via(State))]
struct AppState {
    inner: InnerState,
}

#[derive(Clone, Default, FromRequestParts)]
#[from_request(via(State), state(AppState))]
struct InnerState {}

impl FromRef<AppState> for InnerState {
    fn from_ref(input: &AppState) -> Self {
        input.inner.clone()
    }
}
