use axum::{extract::FromRequestParts, response::Response};

#[derive(FromRequestParts)]
struct Extractor {
    body: String,
}

fn assert_from_request()
where
    Extractor: FromRequestParts<(), Rejection = Response>,
{
}

fn main() {}
