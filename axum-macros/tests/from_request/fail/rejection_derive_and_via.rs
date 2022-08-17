use axum_macros::FromRequest;

#[derive(FromRequest, Clone)]
#[from_request(rejection_derive(!Error), via(axum::Extension))]
struct Extractor {
    config: String,
}

fn main() {}
