use axum_macros::FromRequestParts;

#[derive(FromRequestParts)]
struct Extractor;

fn assert_from_request()
where
    Extractor: axum::extract::FromRequestParts<(), Rejection = std::convert::Infallible>,
{
}

fn main() {}
