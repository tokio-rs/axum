use axum::{
    extract::FromRequestParts,
    response::Response,
};
use axum_extra::{
    TypedHeader,
    typed_header::TypedHeaderRejection,
    headers::{self, UserAgent},
};

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
