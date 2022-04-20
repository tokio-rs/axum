use schemars::JsonSchema;
use axum::Json;
use crate::{Either, openapi};

pub trait DescribeHandlerArg {
    fn describe() -> Option<Either<openapi::Parameter, openapi::RequestBody>>;
}

impl<T: JsonSchema> DescribeHandlerArg for Json<T> {
    fn describe() -> Option<Either<openapi::Parameter, openapi::RequestBody>> {
        let schema = schemars::schema_for!(T);

        Some(Either::Right(openapi::RequestBody {
            description: schema.schema.metadata
                .as_ref()
                .and_then(|meta| {
                    meta.description.clone()
                })
                .unwrap_or_default(),
            content: openapi::RequestBodyContent::Json(schema)
        }))
    }
}