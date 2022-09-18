use axum::{
    extract::{State, FromRef},
    routing::get,
    Router,
};
use axum_macros::FromRequest;

#[tokio::main]
async fn main() {
    let _: Router<AppState> = Router::with_state(AppState::default())
        .route("/", get(|_: Extractor| async {}));
}

#[derive(FromRequest)]
#[from_request(state(AppState))]
struct Extractor {
    #[from_request(via(State))]
    state: AppState,
    #[from_request(via(State))]
    inner: InnerState,
}

#[derive(Clone, Default)]
struct AppState {
    inner: InnerState,
}

#[derive(Clone, Default)]
struct InnerState {}

impl FromRef<AppState> for InnerState {
    fn from_ref(input: &AppState) -> Self {
        input.inner.clone()
    }
}
