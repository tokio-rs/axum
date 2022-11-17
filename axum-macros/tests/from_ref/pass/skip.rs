use axum_macros::FromRef;

#[derive(Clone, FromRef)]
struct AppState {
    auth_token: String,
    #[from_ref(skip)]
    also_string: String,
}

fn main() {}
