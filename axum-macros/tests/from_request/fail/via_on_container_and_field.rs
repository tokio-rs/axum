use axum_macros::FromRequest;

#[derive(FromRequest)]
#[from_request(via(axum::Extension))]
struct Extractor(#[from_request(via(axum::Extension))] State);

#[derive(Clone)]
struct State;

fn main() {}
