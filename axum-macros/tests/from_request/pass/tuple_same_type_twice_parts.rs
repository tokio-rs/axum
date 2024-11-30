use axum::extract::Query;
use axum_macros::FromRequestParts;
use serde::Deserialize;

#[derive(FromRequestParts)]
struct Extractor(Query<Payload>, axum::extract::Path<Payload>);

#[derive(Deserialize)]
struct Payload {}

fn assert_from_request()
where
    Extractor: axum::extract::FromRequestParts<()>,
{
}

fn main() {}
