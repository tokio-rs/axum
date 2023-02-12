use axum::{
    extract::{FromRequest, Json},
    response::Response,
};
use serde::Deserialize;

#[derive(Deserialize, FromRequest)]
#[from_request(via(Json))]
struct Extractor {
    one: i32,
    two: String,
    three: bool,
}

fn assert_from_request()
where
    Extractor: FromRequest<(), Rejection = Response>,
{
}

fn main() {}
