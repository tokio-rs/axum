use std::{future::Future, marker::PhantomData};

use axum::{http::Response, response::IntoResponse, Json};
use okapi::openapi3::{self, MediaType, Operation, RefOr, Responses};
use schemars::JsonSchema;

use crate::ToOperation;

pub trait ToResponses {
    fn to_responses() -> Responses;
}

// `const S: &'static str` is gonna be great
pub trait ResponseDescription {
    const DESCRIPTION: &'static str;
}

pub trait IntoOpenApiResponse: ToResponses + IntoResponse {}

impl<T> IntoOpenApiResponse for T where T: ToResponses + IntoResponse {}

impl<F, Fut, Res> ToOperation<()> for F
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Res> + Send,
    Res: ToResponses,
{
    fn to_operation(&self) -> Operation {
        let mut op = Operation::default();
        op.responses = Res::to_responses();
        op
    }
}

pub struct WithDescription<T, K> {
    response: K,
    _marker: PhantomData<fn() -> T>,
}

#[macro_export]
macro_rules! describe {
    (
        $description:literal,
        $res:expr $(,)?
    ) => {{
        struct __ResponseDescription;
        impl $crate::ResponseDescription for __ResponseDescription {
            const DESCRIPTION: &'static str = $description;
        }
        $crate::describe::<__ResponseDescription, _>($res)
    }};
}

pub fn describe<T, K>(response: K) -> WithDescription<T, K>
where
    T: ResponseDescription,
    K: IntoResponse,
{
    WithDescription {
        response,
        _marker: PhantomData,
    }
}

impl<T, K> IntoResponse for WithDescription<T, K>
where
    K: IntoResponse,
{
    type Body = K::Body;
    type BodyError = K::BodyError;

    fn into_response(self) -> Response<Self::Body> {
        self.response.into_response()
    }
}

impl<T, K> ToResponses for WithDescription<T, K>
where
    T: ResponseDescription,
    K: ToResponses,
{
    fn to_responses() -> Responses {
        let mut res = K::to_responses();
        if let Some(RefOr::Object(default)) = &mut res.default {
            default.description = T::DESCRIPTION.to_string();
        }
        res
    }
}

impl ToResponses for () {
    fn to_responses() -> Responses {
        Responses::default()
    }
}

impl<T> ToResponses for Json<T>
where
    T: JsonSchema,
{
    fn to_responses() -> Responses {
        let schema = schemars::schema_for!(T).schema;
        let mut media_type = MediaType::default();
        media_type.schema = Some(schema);

        let response = openapi3::Response {
            content: vec![("application/json".to_string(), media_type)]
                .into_iter()
                .collect(),
            ..Default::default()
        };

        Responses {
            default: Some(RefOr::Object(response)),
            responses: Default::default(),
            extensions: Default::default(),
        }
    }
}
