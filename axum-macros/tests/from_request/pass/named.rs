use axum::{
    body::Body,
    extract::{FromRequest, TypedHeader, rejection::TypedHeaderRejection},
    response::Response,
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
    Extractor: FromRequest<(), Body, Rejection = Response>,
{
}

fn main() {}
