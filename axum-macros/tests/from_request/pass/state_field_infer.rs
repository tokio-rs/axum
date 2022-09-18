use axum::{
    extract::State,
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
struct Extractor {
    #[from_request(via(State))]
    state: AppState,
}

#[derive(Clone, Default)]
struct AppState {}
