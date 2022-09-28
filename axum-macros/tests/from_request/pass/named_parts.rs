use axum::{
    extract::{rejection::TypedHeaderRejection, FromRequestParts, TypedHeader},
    headers::{self, UserAgent},
    response::Response,
};
use axum_macros::FromRequestParts;

#[derive(FromRequestParts)]
struct Extractor {
    uri: axum::http::Uri,
    user_agent: TypedHeader<UserAgent>,
    content_type: TypedHeader<headers::ContentType>,
    etag: Option<TypedHeader<headers::ETag>>,
    host: Result<TypedHeader<headers::Host>, TypedHeaderRejection>,
}

fn assert_from_request()
where
    Extractor: FromRequestParts<(), Rejection = Response>,
{
}

fn main() {}
