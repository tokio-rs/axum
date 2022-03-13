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

pub trait DescribeResponse {
    fn describe(operation: &mut Operation, components: &mut Components);
}

impl DescribeResponse for () {
    fn describe(operation: &mut Operation, components: &mut Components) {
        Ok::describe(operation, components)
    }
}

macro_rules! impl_tuples {
    ( $($ty:ident),* $(,)? ) => {
        impl<$($ty,)*> DescribeResponse for ($($ty,)*)
        where
            $($ty: DescribeResponse,)*
        {
            fn describe(operation: &mut Operation, components: &mut Components) {
                $( $ty::describe(operation, components); )*
            }
        }
    };
}

all_the_tuples!(impl_tuples);

macro_rules! status {
    (
        $name:ident, $variant:ident
    ) => {
        #[derive(Copy, Clone)]
        pub struct $name;

        impl IntoResponse for $name {
            fn into_response(self) -> Response {
                StatusCode::$variant.into_response()
            }
        }

        impl DescribeResponse for $name {
            fn describe(operation: &mut Operation, _: &mut Components) {
                operation.responses.responses.insert(
                    StatusCode::$variant.as_u16().to_string(),
                    RefOr::Object(openapi3::Response {
                        description: "Successful response".to_owned(),
                        ..Default::default()
                    }),
                );
            }
        }
    };
}

status!(Ok, OK);
status!(Created, CREATED);
