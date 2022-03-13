use axum::{
    body::HttpBody,
    extract::FromRequest,
    handler::Handler,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{self, MethodRouter},
    Json, Router,
};
use okapi::openapi3::{
    self, Components, Info, MediaType, OpenApi, Operation, Parameter, RefOr, RequestBody,
};
use schemars::{
    schema::{RootSchema, Schema},
    JsonSchema,
};
use std::{
    collections::BTreeMap, convert::Infallible, future::Future, marker::PhantomData, sync::Arc,
};

pub trait DescribeRequest {
    fn describe(operation: &mut Operation, components: &mut Components);
}

impl DescribeRequest for () {
    fn describe(_: &mut Operation, _: &mut Components) {}
}

macro_rules! impl_tuples {
    ( $($ty:ident),* $(,)? ) => {
        impl<$($ty,)*> DescribeRequest for ($($ty,)*)
        where
            $($ty: DescribeRequest,)*
        {
            fn describe(operation: &mut Operation, components: &mut Components) {
                $( $ty::describe(operation, components); )*
            }
        }
    };
}

all_the_tuples!(impl_tuples);

impl<T> DescribeRequest for Json<T>
where
    T: JsonSchema,
{
    fn describe(operation: &mut Operation, components: &mut Components) {
        let RootSchema {
            mut schema,
            definitions,
            meta_schema: _,
        } = schemars::schema_for!(T);

        components.schemas.extend(
            definitions
                .into_iter()
                .filter_map(|(k, schema)| match schema {
                    Schema::Bool(_) => None,
                    Schema::Object(obj) => Some((k, obj)),
                }),
        );

        schema.object().properties = std::mem::take(&mut schema.object().properties)
            .into_iter()
            .map(|(key, schema)| match schema {
                Schema::Bool(_) => (key, schema),
                Schema::Object(mut obj) => {
                    if let Some(reference) = &mut obj.reference {
                        *reference = reference.replace("/definitions/", "/components/schemas/");
                    }
                    (key, Schema::Object(obj))
                }
            })
            .collect();

        let request_body = RequestBody {
            content: schemars::Map::from_iter([(
                mime::APPLICATION_JSON.to_string(),
                MediaType {
                    schema: Some(schema),
                    ..Default::default()
                },
            )]),
            required: true,
            ..Default::default()
        };

        operation.request_body = Some(RefOr::Object(request_body));
    }
}
