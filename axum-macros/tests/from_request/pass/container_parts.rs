use axum::{
    extract::{FromRequestParts, Extension},
    response::Response,
};

#[derive(Clone, FromRequestParts)]
#[from_request(via(Extension))]
struct Extractor {
    one: i32,
    two: String,
    three: bool,
}

fn assert_from_request()
where
    Extractor: FromRequestParts<(), Rejection = Response>,
{
}

fn main() {}
