use super::IntoResponse;
use crate::body::Body;

macro_rules! define_rejection {
    (
        #[status = $status:ident]
        #[body = $body:expr]
        pub struct $name:ident (());
    ) => {
        #[derive(Debug)]
        pub struct $name(pub(super) ());

        impl IntoResponse<Body> for $name {
            fn into_response(self) -> http::Response<Body> {
                let mut res = http::Response::new(Body::from($body));
                *res.status_mut() = http::StatusCode::$status;
                res
            }
        }
    };

    (
        #[status = $status:ident]
        #[body = $body:expr]
        pub struct $name:ident (BoxError);
    ) => {
        #[derive(Debug)]
        pub struct $name(pub(super) tower::BoxError);

        impl $name {
            pub(super) fn from_err<E>(err: E) -> Self
            where
                E: Into<tower::BoxError>,
            {
                Self(err.into())
            }
        }

        impl IntoResponse<Body> for $name {
            fn into_response(self) -> http::Response<Body> {
                let mut res =
                    http::Response::new(Body::from(format!(concat!($body, ": {}"), self.0)));
                *res.status_mut() = http::StatusCode::$status;
                res
            }
        }
    };
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Query string was invalid or missing"]
    pub struct QueryStringMissing(());
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to parse the response body as JSON"]
    pub struct InvalidJsonBody(BoxError);
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Expected request with `Content-Type: application/json`"]
    pub struct MissingJsonContentType(());
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "Missing request extension"]
    pub struct MissingExtension(());
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to buffer the request body"]
    pub struct FailedToBufferBody(BoxError);
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Response body didn't contain valid UTF-8"]
    pub struct InvalidUtf8(BoxError);
}

define_rejection! {
    #[status = PAYLOAD_TOO_LARGE]
    #[body = "Request payload is too large"]
    pub struct PayloadTooLarge(());
}

define_rejection! {
    #[status = LENGTH_REQUIRED]
    #[body = "Content length header is required"]
    pub struct LengthRequired(());
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "No url params found for matched route. This is a bug in tower-web. Please open an issue"]
    pub struct MissingRouteParams(());
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "Cannot have two request body extractors for a single handler"]
    pub struct BodyAlreadyTaken(());
}
