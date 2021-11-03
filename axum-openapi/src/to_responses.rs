use crate::ToOperation;
use axum::{http::Response, response::IntoResponse, Json};
use okapi::openapi3::{self, Components, MediaType, Operation, RefOr, Responses, SchemaObject};
use schemars::{schema::RootSchema, JsonSchema};
use std::{future::Future, marker::PhantomData};

pub trait ToResponses {
    fn to_responses(components: &mut Components) -> Responses;
}

// `const S: &'static str` is gonna be great
pub trait ResponseDescription {
    const DESCRIPTION: &'static str;
}

pub trait IntoOpenApiResponse: ToResponses + IntoResponse {}

impl<T> IntoOpenApiResponse for T where T: ToResponses + IntoResponse {}

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
    fn to_responses(components: &mut Components) -> Responses {
        let mut res = K::to_responses(components);
        if let Some(RefOr::Object(default)) = &mut res.default {
            default.description = T::DESCRIPTION.to_string();
        }
        res
    }
}

impl ToResponses for () {
    fn to_responses(_components: &mut Components) -> Responses {
        Responses::default()
    }
}

impl<T> ToResponses for Json<T>
where
    T: JsonSchema,
{
    fn to_responses(components: &mut Components) -> Responses {
        let gen = schemars::gen::SchemaSettings::openapi3().into_generator();

        let RootSchema {
            // TODO(david): what is `meta_schema`?
            meta_schema,
            schema,
            definitions,
        } = gen.into_root_schema_for::<T>();

        let mut media_type = MediaType::default();
        media_type.schema = Some(schema);

        let definitions = definitions
            .into_iter()
            .filter_map(|(k, schema)| match schema {
                schemars::schema::Schema::Bool(_) => None,
                schemars::schema::Schema::Object(obj) => Some((k, obj)),
            })
            .collect::<schemars::Map<_, _>>();
        components.schemas.extend(definitions);

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
