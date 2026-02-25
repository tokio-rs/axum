use axum::{
    extract::FromRequestParts,
    response::{IntoResponse, Response},
};
use axum_extra::{
    headers,
    typed_header::TypedHeaderRejection,
    TypedHeader,
};

// Option<T> with via() in a FromRequestParts derive should use
// OptionalFromRequestParts, not .ok().
#[derive(FromRequestParts)]
#[from_request(rejection(MyError))]
struct Extractor {
    #[from_request(via(TypedHeader))]
    content_type: Option<headers::ContentType>,
    #[from_request(via(TypedHeader))]
    user_agent: headers::UserAgent,
}

fn assert_from_request()
where
    Extractor: FromRequestParts<(), Rejection = MyError>,
{
}

struct MyError(Response);

impl From<TypedHeaderRejection> for MyError {
    fn from(rejection: TypedHeaderRejection) -> Self {
        Self(rejection.into_response())
    }
}

impl IntoResponse for MyError {
    fn into_response(self) -> Response {
        self.0
    }
}

fn main() {}
