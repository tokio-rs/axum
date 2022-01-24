use axum_macros::FromRequest;

#[derive(FromRequest)]
struct Extractor {
    thing: bool,
}

fn main() {}
