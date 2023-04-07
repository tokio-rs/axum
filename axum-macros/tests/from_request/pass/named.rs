use axum::{
    extract::FromRequest,
    response::Response,
};
use axum_extra::{
    TypedHeader,
    typed_header::TypedHeaderRejection,
    headers::{self, UserAgent},
};

#[derive(FromRequest)]
struct Extractor {
    uri: axum::http::Uri,
    user_agent: TypedHeader<UserAgent>,
    content_type: TypedHeader<headers::ContentType>,
    etag: Option<TypedHeader<headers::ETag>>,
    host: Result<TypedHeader<headers::Host>, TypedHeaderRejection>,
    body: String,
}

fn assert_from_request()
where
    Extractor: FromRequest<(), Rejection = Response>,
{
}

fn main() {}
