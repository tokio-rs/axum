#![allow(
    clippy::field_reassign_with_default,
    clippy::new_without_default,
    clippy::type_complexity,
    dead_code,
    unreachable_code,
    unused_imports,
    unused_mut,
    unused_variables
)]

use axum::{
    body::{Body, BoxBody, Bytes, Full, HttpBody},
    http::{header, HeaderValue, Request, Response, StatusCode},
    response::IntoResponse,
    routing::Route,
    BoxError, Json, Router,
};
use openapiv3::{HeaderStyle, MediaType, OpenAPI, Operation, PathItem, ReferenceOr, Responses};
use std::{borrow::Cow, convert::Infallible, future::Future};
use tower_layer::Layer;
use tower_service::Service;

pub mod handler_method_routing;

pub struct OpenApiRouter<B = Body> {
    router: Router<B>,
    schema: OpenAPI,
}

impl<B> OpenApiRouter<B>
where
    B: Send + 'static,
{
    pub fn new() -> Self {
        Self::with_openapi(Default::default())
    }

    pub fn with_openapi(openapi: OpenAPI) -> Self {
        Self {
            router: Default::default(),
            schema: openapi,
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
            .insert(path.to_string(), ReferenceOr::Item(service.to_path_item()));

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

    pub fn into_parts(self) -> (Router<B>, OpenAPI) {
        (self.router, self.schema)
    }
}

pub fn schema_routes(schema: OpenAPI) -> SchemaRoutes {
    SchemaRoutes {
        schema,
        json: true,
        yaml: true,
        path: "openapi".into(),
    }
}

pub struct SchemaRoutes {
    schema: OpenAPI,
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
                axum::routing::get(|Extension(schema): Extension<Arc<OpenAPI>>| async move {
                    Yaml((&*schema).clone())
                }),
            );
        }

        if self.json {
            router = router.route(
                &format!("/{}.yaml", self.path),
                axum::routing::get(|Extension(schema): Extension<Arc<OpenAPI>>| async move {
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

pub trait ToPathItem {
    fn to_path_item(&self) -> PathItem;
}

pub trait ToOperation<T> {
    fn to_operation(&self) -> Operation;
}

pub trait ToResponses {
    fn to_responses() -> Responses;
}

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

impl ToResponses for () {
    fn to_responses() -> Responses {
        Responses::default()
    }
}

impl<T> ToResponses for Json<T> {
    fn to_responses() -> Responses {
        let response = openapiv3::Response {
            content: vec![("application/json".to_string(), MediaType::default())]
                .into_iter()
                .collect(),
            ..Default::default()
        };

        // TODO(david): how to handle `response.content.schema`? Would be cool if we could say
        // something else than "its JSON"

        Responses {
            default: Some(ReferenceOr::Item(response)),
            responses: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler_method_routing::get;
    use axum::{
        extract::Extension,
        http::{header::CONTENT_TYPE, StatusCode},
        response::Headers,
        AddExtensionLayer, Json,
    };
    use serde_json::json;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_openapi() {
        let app = OpenApiRouter::new()
            .route("/", get(|| async { Json(json!({ "foo": "bar" })) }))
            .route("/foo", get(|| async {}).post(|| async {}));

        let (router, openapi) = app.into_parts();

        println!("{}", serde_yaml::to_string(&openapi).unwrap());

        let router_with_openapi_schema = router.merge(schema_routes(openapi).into_router());
    }
}
