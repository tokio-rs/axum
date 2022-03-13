#![allow(missing_debug_implementations, dead_code, unused_imports)]
#![deny(unreachable_pub)]

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

#[macro_use]
mod macros {
    macro_rules! all_the_tuples {
        ($name:ident) => {
            $name!(T1);
            $name!(T1, T2);
            $name!(T1, T2, T3);
            $name!(T1, T2, T3, T4);
            $name!(T1, T2, T3, T4, T5);
            $name!(T1, T2, T3, T4, T5, T6);
            $name!(T1, T2, T3, T4, T5, T6, T7);
            $name!(T1, T2, T3, T4, T5, T6, T7, T8);
            $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
            $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
            $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
            $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
            $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
            $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
            $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
            $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);
        };
    }
}

mod describe_request;
mod describe_response;

pub use self::{
    describe_request::DescribeRequest,
    describe_response::{Created, DescribeResponse, Ok},
};

pub struct OpenApiRouter<B> {
    router: Router<B>,
    schema: OpenApi,
}

impl<B> OpenApiRouter<B>
where
    B: HttpBody + Send + 'static,
{
    pub fn new(info: Info) -> Self {
        Self {
            router: Default::default(),
            schema: OpenApi {
                info,
                openapi: "3.0.0".to_owned(),
                components: Some(Components::default()),
                ..Default::default()
            },
        }
    }

    pub fn into_parts(self) -> (Router<B>, OpenApi) {
        (self.router, self.schema)
    }

    pub fn route(mut self, path: &str, handler: OpenApiHandler<B>) -> Self {
        let OpenApiHandler {
            svc,
            method,
            operation,
            components,
        } = handler;

        self.router = self.router.route(path, svc);

        extend_components(self.schema.components.as_mut().unwrap(), components);

        let path = path
            .split('/')
            .map(|segment| {
                if let Some(param) = segment.strip_prefix(':') {
                    format!("{{{}}}", param)
                } else {
                    // TODO(david): wildcards
                    segment.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("/");

        let path_item = self.schema.paths.entry(path).or_default();
        match method {
            Method::Get => path_item.get = Some(operation),
            Method::Post => path_item.post = Some(operation),
        }

        self
    }
}

fn extend_components(current: &mut Components, new: Components) {
    let Components {
        schemas,
        responses,
        parameters,
        examples,
        request_bodies,
        headers,
        security_schemes,
        links,
        callbacks,
        extensions,
    } = current;

    schemas.extend(new.schemas);
    responses.extend(new.responses);
    parameters.extend(new.parameters);
    examples.extend(new.examples);
    request_bodies.extend(new.request_bodies);
    headers.extend(new.headers);
    security_schemes.extend(new.security_schemes);
    links.extend(new.links);
    callbacks.extend(new.callbacks);
    extensions.extend(new.extensions);
}

macro_rules! method {
    ($fn_name:ident, $method:ident) => {
        pub fn $fn_name<H, T, B>(handler: H) -> OpenApiHandler<B>
        where
            H: Handler<T, B> + HandlerResponse<T>,
            T: DescribeRequest + 'static,
            H::Response: DescribeResponse,
            B: Send + 'static,
        {
            let mut operation = Operation::default();
            let mut components = Components::default();
            T::describe(&mut operation, &mut components);
            H::Response::describe(&mut operation, &mut components);
            OpenApiHandler {
                svc: routing::$fn_name(handler),
                method: Method::$method,
                operation,
                components,
            }
        }
    };
}

method!(get, Get);
method!(post, Post);

// TODO(david): the remaining methods
enum Method {
    Get,
    Post,
}

pub struct OpenApiHandler<B> {
    svc: MethodRouter<B, Infallible>,
    method: Method,
    operation: Operation,
    components: Components,
}

impl<B> OpenApiHandler<B> {
    pub fn operation_id<S>(self, id: S) -> Self
    where
        S: Into<String>,
    {
        self.map_operation(|op, _| {
            op.operation_id = Some(id.into());
        })
    }

    pub fn summary<S>(self, summary: S) -> Self
    where
        S: Into<String>,
    {
        self.map_operation(|op, _| {
            op.summary = Some(summary.into());
        })
    }

    pub fn description<S>(self, description: S) -> Self
    where
        S: Into<String>,
    {
        self.map_operation(|op, _| {
            op.description = Some(description.into());
        })
    }

    pub fn map_operation<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut Operation, &mut Components),
    {
        f(&mut self.operation, &mut self.components);
        self
    }
}

pub trait HandlerResponse<T> {
    type Response;
}

impl<F, Fut> HandlerResponse<()> for F
where
    F: FnOnce() -> Fut,
    Fut: Future,
{
    type Response = Fut::Output;
}

macro_rules! impl_tuples {
    ( $($ty:ident),* $(,)? ) => {
        impl<F, Fut, $($ty,)*> HandlerResponse<($($ty,)*)> for F
        where
            F: FnOnce($($ty,)*) -> Fut,
            Fut: Future,
        {
            type Response = Fut::Output;
        }
    };
}

all_the_tuples!(impl_tuples);

#[cfg(test)]
mod tests {
    use super::*;
    use assert_json_diff::assert_json_eq;
    use axum::body::Body;
    use serde::Deserialize;
    use serde_json::json;

    #[test]
    fn test_something() {
        #[derive(Deserialize, JsonSchema)]
        struct UsersCreate {
            account: Account,
        }

        #[derive(Deserialize, JsonSchema)]
        struct Account {
            username: String,
        }

        async fn users_show() {}

        async fn users_create(Json(_): Json<UsersCreate>) -> Created {
            Created
        }

        let (router, schema) = OpenApiRouter::<Body>::new(Info::default())
            .route("/users/:id", get(users_show).operation_id("users_show"))
            .route("/users", post(users_create).operation_id("users_create"))
            .into_parts();

        assert_json_eq!(
            schema,
            json!({
                "openapi": "3.0.0",
                "info": {
                    "title": "",
                    "version": ""
                },
                "paths": {
                    "/users": {
                        "post": {
                            "operationId": "users_create",
                            "requestBody": {
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "title": "UsersCreate",
                                            "type": "object",
                                            "required": [
                                                "account",
                                            ],
                                            "properties": {
                                                "account": {
                                                    "$ref": "#/components/schemas/Account"
                                                }
                                            }
                                        }
                                    }
                                },
                                "required": true
                            },
                            "responses": {
                                "201": {
                                    "description": "Successful response",
                                }
                            }
                        }
                    },
                    "/users/{id}": {
                        "get": {
                            "operationId": "users_show",
                            "responses": {
                                "200": {
                                    "description": "Successful response",
                                }
                            }
                        }
                    }
                },
                "components": {
                    "schemas": {
                        "Account": {
                            "type": "object",
                            "required": [
                                "username"
                            ],
                            "properties": {
                                "username": {
                                    "type": "string"
                                }
                            }
                        }
                    }
                }
            }),
        );
    }
}
