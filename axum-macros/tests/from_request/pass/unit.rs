use axum_macros::FromRequest;

#[derive(FromRequest)]
struct Extractor;

fn assert_from_request()
where
    Extractor: axum::extract::FromRequest<axum::body::Body>,
{
}

fn main() {}
