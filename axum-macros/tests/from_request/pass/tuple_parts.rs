use axum_macros::FromRequestParts;

#[derive(FromRequestParts)]
struct Extractor(axum::http::HeaderMap, axum::http::Method);

fn assert_from_request()
where
    Extractor: axum::extract::FromRequestParts<()>,
{
}

fn main() {}
