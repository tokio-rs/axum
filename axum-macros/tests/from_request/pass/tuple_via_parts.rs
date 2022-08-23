use axum::Extension;
use axum_macros::FromRequestParts;

#[derive(FromRequestParts)]
struct Extractor(#[from_request(via(Extension))] State);

#[derive(Clone)]
struct State;

fn assert_from_request()
where
    Extractor: axum::extract::FromRequestParts<()>,
{
}

fn main() {}
