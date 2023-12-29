use axum::{
    extract::{rejection::ExtensionRejection, FromRequestParts},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Extension, Router,
};

fn main() {
    let _: Router = Router::new().route("/", get(handler).post(handler_result));
}

async fn handler(_: MyExtractor) {}

async fn handler_result(_: Result<MyExtractor, MyRejection>) {}

#[derive(FromRequestParts)]
#[from_request(rejection(MyRejection))]
struct MyExtractor {
    one: Extension<String>,
    #[from_request(via(Extension))]
    two: String,
    three: OtherExtractor,
}

struct OtherExtractor;

impl<S> FromRequestParts<S> for OtherExtractor
where
    S: Send + Sync,
{
    // this rejection doesn't implement `Display` and `Error`
    type Rejection = (StatusCode, String);

    async fn from_request_parts(_parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

struct MyRejection {}

impl From<ExtensionRejection> for MyRejection {
    fn from(_: ExtensionRejection) -> Self {
        todo!()
    }
}

impl From<(StatusCode, String)> for MyRejection {
    fn from(_: (StatusCode, String)) -> Self {
        todo!()
    }
}

impl IntoResponse for MyRejection {
    fn into_response(self) -> Response {
        todo!()
    }
}
