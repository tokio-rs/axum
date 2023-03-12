use axum_macros::FromRequest;

#[derive(FromRequest)]
struct Extractor;

fn assert_from_request()
where
    Extractor: axum::extract::FromRequest<(), Rejection = std::convert::Infallible>,
{
}

fn main() {}
