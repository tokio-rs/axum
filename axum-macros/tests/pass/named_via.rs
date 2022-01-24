use axum::{body::Body, extract::{FromRequest, TypedHeader}, headers::{self, UserAgent}};
use axum_macros::FromRequest;
use std::convert::Infallible;

#[derive(FromRequest)]
struct Extractor {
    uri: axum::http::Uri,
    #[from_request(via(TypedHeader))]
    user_agent: UserAgent,
    #[from_request(via(TypedHeader))]
    content_type: headers::ContentType,
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
        ExtractorRejection::Uri(inner) => {
            let _: Infallible = inner;
        }
        ExtractorRejection::String(inner) => {
            let _: axum::extract::rejection::StringRejection = inner;
        }
        ExtractorRejection::UserAgent(inner) => {
            let _: axum::extract::rejection::TypedHeaderRejection = inner;
        }
        ExtractorRejection::ContentType(inner) => {
            let _: axum::extract::rejection::TypedHeaderRejection = inner;
        }
    }
}

#[derive(Clone)]
struct State;

fn main() {}
