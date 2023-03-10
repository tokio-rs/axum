use axum::{
    body::Body,
    response::Response,
    extract::{
        rejection::TypedHeaderRejection,
        Extension, FromRequest, TypedHeader,
    },
    headers::{self, UserAgent},
};

#[derive(FromRequest)]
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
    Extractor: FromRequest<(), Body, Rejection = Response>,
{
}

#[derive(Clone)]
struct State;

fn main() {}
