use axum::{extract::FromRequestParts, response::Response};
use axum_macros::FromRequestParts;

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
