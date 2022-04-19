use std::ops::Deref;
use either::Either;
use axum::handler::Handler;
use axum::http::Request;

pub mod openapi;

pub mod routing;

use axum::Json;

pub use axum_openapi_macros::{route, JsonBody};

/// A decorated handler created by [`#[route]`][route], capable of providing OpenAPI documentation.
#[derive(Clone)]
pub struct RouteHandler<H> {
    pub handler: H,
    pub describe: fn() -> openapi::Operation,
}

impl<T, B, H: Handler<T, B>> Handler<T, B> for RouteHandler<H> {
    type Future = H::Future;

    fn call(self, req: Request<B>) -> Self::Future {
        self.handler.call(req)
    }
}

pub trait HandlerArg {
    fn describe() -> Option<Either<openapi::Parameter, openapi::RequestBody>>;
}

pub enum HandlerArgKind {
    Parameter(openapi::Parameter),
    RequestBody(openapi::RequestBody),
}

pub trait JsonBody {
    fn description() -> &'static str;

    fn json_schema() -> schemars::schema::RootSchema;
}

impl<T: JsonBody> HandlerArg for Json<T> {
    fn describe() -> Option<Either<openapi::Parameter, openapi::RequestBody>> {
        Some(Either::Right(openapi::RequestBody {
            description: "",
            content: openapi::RequestBodyContent::Json(T::json_schema()),
        }))
    }
}


#[doc(hidden)]
pub mod __macro_reexport {
    pub use axum::handler::Handler;
    pub use axum::response::Response;
    pub use futures_core::future::BoxFuture;

    pub use schemars;
}