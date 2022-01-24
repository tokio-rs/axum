use axum::{body::Body, extract::FromRequest};
use axum_macros::FromRequest;

#[derive(FromRequest)]
struct Extractor {
    headers: axum::http::HeaderMap,
    body: String,
}

fn assert_from_request()
where
    Extractor: FromRequest<Body, Rejection = ExtractorRejection>,
{
}

fn assert_rejection(rejection: ExtractorRejection)
where
    ExtractorRejection: std::fmt::Debug + std::fmt::Display + std::error::Error,
{
    match rejection {
        ExtractorRejection::HeaderMap(inner) => {
            let _: std::convert::Infallible = inner;
        }
        ExtractorRejection::String(inner) => {
            let _: axum::extract::rejection::StringRejection = inner;
        }
    }
}

fn main() {}
