use axum::extract::Query;
use axum::response::Response;
use axum_macros::FromRequestParts;
use serde::Deserialize;

#[derive(FromRequestParts)]
struct Extractor(
    #[from_request(via(Query))] Payload,
    #[from_request(via(axum::extract::Path))] Payload,
);

#[derive(Deserialize)]
struct Payload {}

fn assert_from_request()
where
    Extractor: axum::extract::FromRequestParts<(), Rejection = Response>,
{
}

fn main() {}
