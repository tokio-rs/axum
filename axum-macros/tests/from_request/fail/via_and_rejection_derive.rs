use axum_macros::FromRequest;

#[derive(FromRequest, Clone)]
#[from_request(via(axum::Extension), rejection_derive(!Error))]
struct Extractor {
    config: String,
}

fn main() {}
