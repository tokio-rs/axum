use axum::extract::FromRef;

#[derive(Clone, FromRef)]
struct AppState<T> {
    foo: T,
}

fn main() {}
