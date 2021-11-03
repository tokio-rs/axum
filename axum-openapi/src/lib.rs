#![allow(
    clippy::field_reassign_with_default,
    clippy::new_without_default,
    clippy::type_complexity,
    unreachable_code,
    unused_imports,
    unused_mut
)]

use axum::{
    body::{Body, BoxBody, Bytes, Full, HttpBody},
    http::{header, HeaderValue, Request, Response, StatusCode},
    response::IntoResponse,
    routing::Route,
    BoxError, Json, Router,
};
use okapi::openapi3::{
    self, Components, Info, MediaType, OpenApi, Operation, PathItem, RefOr, Responses,
};
use schemars::JsonSchema;
use std::{borrow::Cow, convert::Infallible, future::Future, marker::PhantomData};
use tower_layer::Layer;
use tower_service::Service;

pub mod handler_method_routing;
mod to_operation;
mod to_path_item;
mod to_responses;

pub use self::{
    to_operation::ToOperation,
    to_path_item::ToPathItem,
    to_responses::{describe, IntoOpenApiResponse, ResponseDescription, ToResponses},
};

pub struct OpenApiRouter<B = Body> {
    router: Router<B>,
    schema: OpenApi,
    components: Components,
}

impl<B> OpenApiRouter<B>
where
    B: Send + 'static,
{
    pub fn new(info: Info) -> Self {
        let schema = OpenApi {
            openapi: OpenApi::default_version(),
            info,
            ..Default::default()
        };
        Self {
            router: Default::default(),
            schema,
            components: Default::default(),
        }
    }

    pub fn route<T>(mut self, path: &str, service: T) -> Self
    where
        T: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible>
            + ToPathItem
            + Clone
            + Send
            + 'static,
        T::Future: Send + 'static,
    {
        // TODO(david): correct path templating with `{key}` instead of `:key`
        // TODO(david): does openapi have wildcards?

        self.schema
            .paths
            .insert(path.to_string(), service.to_path_item(&mut self.components));

        self.router = self.router.route(path, service);
        self
    }

    pub fn nest<T>(mut self, path: &str, service: T) -> Self
    where
        T: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible>
            + Clone
            + Send
            + 'static,
        T::Future: Send + 'static,
    {
        self.router = self.router.nest(path, service);
        self
    }

    pub fn merge(mut self, other: Router<B>) -> Self {
        self.router = self.router.merge(other);
        self
    }

    pub fn layer<L, LayeredReqBody, LayeredResBody>(self, layer: L) -> OpenApiRouter<LayeredReqBody>
    where
        L: Layer<Route<B>>,
        L::Service: Service<
                Request<LayeredReqBody>,
                Response = Response<LayeredResBody>,
                Error = Infallible,
            > + Clone
            + Send
            + 'static,
        <L::Service as Service<Request<LayeredReqBody>>>::Future: Send + 'static,
        LayeredResBody: HttpBody<Data = Bytes> + Send + 'static,
        LayeredResBody::Error: Into<BoxError>,
    {
        OpenApiRouter {
            router: self.router.layer(layer),
            schema: self.schema,
            components: self.components,
        }
    }

    pub fn fallback<T>(mut self, service: T) -> Self
    where
        T: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible>
            + Clone
            + Send
            + 'static,
        T::Future: Send + 'static,
    {
        self.router = self.router.fallback(service);
        self
    }

    pub fn into_parts(mut self) -> (Router<B>, OpenApi) {
        self.schema.components = Some(self.components);
        (self.router, self.schema)
    }
}

pub fn schema_routes(schema: OpenApi) -> SchemaRoutes {
    SchemaRoutes {
        schema,
        json: true,
        yaml: true,
        path: "openapi".into(),
    }
}

pub struct SchemaRoutes {
    schema: OpenApi,
    json: bool,
    yaml: bool,
    path: Cow<'static, str>,
}

impl SchemaRoutes {
    pub fn json(mut self, enabled: bool) -> Self {
        self.json = enabled;
        self
    }

    pub fn yaml(mut self, enabled: bool) -> Self {
        self.yaml = enabled;
        self
    }

    pub fn path(mut self, path: &str) -> Self {
        self.path = path.to_string().into();
        self
    }

    pub fn into_router(self) -> Router {
        use crate::handler_method_routing::get;
        use axum::{
            extract::Extension,
            http::{header::CONTENT_TYPE, StatusCode},
            response::Headers,
            AddExtensionLayer, Json,
        };
        use std::sync::Arc;

        let mut router = Router::new();

        if self.yaml {
            router = router.route(
                &format!("/{}.yaml", self.path),
                axum::routing::get(|Extension(schema): Extension<Arc<OpenApi>>| async move {
                    Yaml((&*schema).clone())
                }),
            );
        }

        if self.json {
            router = router.route(
                &format!("/{}.json", self.path),
                axum::routing::get(|Extension(schema): Extension<Arc<OpenApi>>| async move {
                    Json((&*schema).clone())
                }),
            )
        }

        router.layer(AddExtensionLayer::new(Arc::new(self.schema)))
    }
}

struct Yaml<T>(T);

impl<T> IntoResponse for Yaml<T>
where
    T: serde::Serialize,
{
    type Body = Full<Bytes>;
    type BodyError = <Self::Body as HttpBody>::Error;

    fn into_response(self) -> Response<Self::Body> {
        let bytes = match serde_yaml::to_vec(&self.0) {
            Ok(bytes) => bytes,
            Err(err) => {
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Full::from(err.to_string()))
                    .unwrap();
            }
        };

        let mut res = Response::new(Full::from(bytes));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/x-yaml"),
        );
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler_method_routing::get;
    use assert_json_diff::assert_json_eq;
    use axum::{
        extract::{Extension, Path},
        http::{header::CONTENT_TYPE, StatusCode},
        response::Headers,
        AddExtensionLayer, Json,
    };
    use serde::Serialize;
    use serde_json::json;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_openapi() {
        #[derive(Serialize, JsonSchema)]
        struct RootResponse {
            foo: &'static str,
            bar: Inner,
        }

        #[derive(Serialize, JsonSchema)]
        struct Inner {
            foo: &'static str,
            bar: bool,
        }

        async fn root() -> impl IntoOpenApiResponse {
            describe!(
                "Just JSON",
                Json(RootResponse {
                    foo: "hi",
                    bar: Inner {
                        foo: "hi",
                        bar: false,
                    },
                })
            )
        }

        async fn get_foo(Path(id): Path<u64>) -> impl IntoOpenApiResponse {
            // ...
        }

        let info = Info {
            title: "axum-test".to_string(),
            version: "0.0.0".to_string(),
            ..Default::default()
        };

        let app = OpenApiRouter::<Body>::new(info)
            .route("/", get("root", root))
            .route("/foo/:id", get("get_foo", get_foo));

        let (_, openapi) = app.into_parts();

        println!("{}", serde_json::to_string_pretty(&openapi).unwrap());

        // TODO(david): switch to cargo-insta
        assert_json_eq!(
            openapi,
            json!({
                "openapi": "3.0.0",
                "info": {
                    "title": "axum-test",
                    "version": "0.0.0",
                },
                "paths": {
                    "/": {
                        "get": {
                            "operationId": "root",
                            "responses": {
                                "default": {
                                    "description": "Just JSON",
                                    "content": {
                                        "application/json": {
                                            "schema": {
                                                "title": "RootResponse",
                                                "type": "object",
                                                "required": ["bar", "foo"],
                                                "properties": {
                                                    "foo": { "type": "string" },
                                                    "bar": { "$ref": "#/components/schemas/Inner" },
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                        }
                    },
                    "/foo/:id": {
                        "get": {
                            "operationId": "get_foo",
                            "responses": {},
                        },
                    },
                },
                "components": {
                    "schemas": {
                        "Inner": {
                            "type": "object",
                            "required": ["bar", "foo"],
                            "properties": {
                                "foo": { "type": "string" },
                                "bar": { "type": "boolean" },
                            }
                        }
                    }
                }
            }),
        );
    }
}
