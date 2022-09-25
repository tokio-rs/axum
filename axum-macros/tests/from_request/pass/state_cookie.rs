use axum_macros::FromRequest;
use axum::extract::FromRef;
use axum_extra::extract::cookie::{PrivateCookieJar, Key};

#[derive(FromRequest)]
#[from_request(state(AppState))]
struct Extractor {
    cookies: PrivateCookieJar,
}

struct AppState {
    key: Key,
}

impl FromRef<AppState> for Key {
    fn from_ref(input: &AppState) -> Self {
        input.key.clone()
    }
}

fn assert_from_request()
where
    Extractor: axum::extract::FromRequest<AppState, axum::body::Body, Rejection = axum::response::Response>,
{
}

fn main() {}
