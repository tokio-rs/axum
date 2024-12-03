use axum::{
    extract::{Extension, FromRequestParts},
    response::Response,
};
use axum_extra::{
    headers::{self, UserAgent},
    typed_header::TypedHeaderRejection,
    TypedHeader,
};

#[derive(FromRequestParts)]
struct Extractor {
    #[from_request(via(Extension))]
    state: State,
    #[from_request(via(TypedHeader))]
    user_agent: UserAgent,
    #[from_request(via(TypedHeader))]
    content_type: headers::ContentType,
    #[from_request(via(TypedHeader))]
    etag: Option<headers::ETag>,
    #[from_request(via(TypedHeader))]
    host: Result<headers::Host, TypedHeaderRejection>,
}

fn assert_from_request()
where
    Extractor: FromRequestParts<(), Rejection = Response>,
{
}

#[derive(Clone)]
struct State;

fn main() {}
