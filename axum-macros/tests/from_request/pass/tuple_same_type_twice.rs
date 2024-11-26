use axum::extract::Query;
use axum_macros::FromRequest;
use serde::Deserialize;

#[derive(FromRequest)]
struct Extractor(Query<Payload>, axum::extract::Json<Payload>);

#[derive(Deserialize)]
struct Payload {}

fn assert_from_request()
where
    Extractor: axum::extract::FromRequest<()>,
{
}

fn main() {}
