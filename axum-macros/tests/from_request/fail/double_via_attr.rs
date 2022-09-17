use axum_macros::FromRequest;

#[derive(FromRequest)]
struct Extractor(#[from_request(via(axum::Extension), via(axum::Extension))] State);

#[derive(Clone)]
struct State;

fn main() {}
