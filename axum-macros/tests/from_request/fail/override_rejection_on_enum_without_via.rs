use axum::{
    extract::rejection::ExtensionRejection,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use axum_macros::FromRequest;

fn main() {
    let _: Router = Router::new().route("/", get(handler).post(handler_result));
}

async fn handler(_: MyExtractor) {}

async fn handler_result(_: Result<MyExtractor, MyRejection>) {}

#[derive(FromRequest, Clone)]
#[from_request(rejection(MyRejection))]
enum MyExtractor {}

struct MyRejection {}

impl From<ExtensionRejection> for MyRejection {
    fn from(_: ExtensionRejection) -> Self {
        todo!()
    }
}

impl IntoResponse for MyRejection {
    fn into_response(self) -> Response {
        todo!()
    }
}
