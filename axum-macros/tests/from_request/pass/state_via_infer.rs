use axum::{
    extract::State,
    routing::get,
    Router,
};
use axum_macros::FromRequest;

#[tokio::main]
async fn main() {
    let _: Router<AppState> = Router::with_state(AppState::default())
        .route("/b", get(|_: AppState| async {}));
}

// if we're extract "via" `State<AppState>` and not specifying state
// assume `AppState` is the state
#[derive(Clone, Default, FromRequest)]
#[from_request(via(State))]
struct AppState {}
