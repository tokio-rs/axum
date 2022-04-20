use std::convert::Infallible;
use std::sync::Arc;
use indexmap::IndexMap;
use serde_json::value::RawValue;
use axum::body::{Body, HttpBody};
use axum::{Extension, Json};
use axum::handler::Handler;
use axum::http::Request;
use axum::response::Response;

use crate::{openapi};
use crate::handler::{DescribeHandler, DocumentedHandler};
use crate::openapi::OpenApi;

pub struct Router<B = Body> {
    inner: axum::routing::Router<B>,
    paths: IndexMap<String, openapi::PathItem>
}

impl<B> Router<B> where B: HttpBody + Send + 'static {
    pub fn new() -> Self {
        Self {
            inner: axum::routing::Router::new(),
            paths: IndexMap::new(),
        }
    }

    pub fn route(mut self, path: &str, service: MethodRouter<B, Infallible>) -> Self {
        let inner = self.inner.route(path, service.inner);
        self.paths.insert(path.to_string(), service.path_item);

        Self {
            inner,
            paths: self.paths
        }
    }

    pub fn spec(self, title: impl Into<String>) -> SpecBuilder<B> {
        SpecBuilder {
            router: self.inner,
            info: openapi::Info {
                title: title.into(),
                ..Default::default()
            },
            paths: self.paths,
        }
    }

    pub fn discard_spec(self) -> axum::routing::Router<B> {
        self.inner
    }
}

pub struct MethodRouter<B = Body, E = Infallible> {
    inner: axum::routing::MethodRouter<B, E>,
    path_item: openapi::PathItem
}

impl<B, E> MethodRouter<B, E> {
    pub fn new() -> Self {
        MethodRouter {
            inner: axum::routing::MethodRouter::new(),
            path_item: Default::default()
        }
    }
}

impl<B: Send + 'static> MethodRouter<B, Infallible> {
    pub fn post<T, H>(self, handler: DocumentedHandler<H>) -> Self
    where H: Handler<T, B> + DescribeHandler<T>, T: Send + 'static
    {
        let operation = handler.to_operation();

        Self {
            inner: self.inner.post(handler.handler),
            path_item: openapi::PathItem {
                post: Some(operation),
                ..self.path_item
            }
        }
    }
}

pub struct SpecBuilder<B = Body> {
    router: axum::routing::Router<B>,
    info: openapi::Info,
    paths: IndexMap<String, openapi::PathItem>,
}

impl<B> SpecBuilder<B> where B: HttpBody + Send + 'static {
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.info.description = Some(description.into());
        self
    }

    pub fn serve_at(self, path: &str) -> axum::routing::Router<B> {
        let openapi = OpenApi {
            openapi: "3.0.3",
            info: self.info,
            paths: self.paths,
        };

        // eagerly serialize the response

        let serialized: Arc<RawValue> = serde_json::value::to_raw_value(&openapi)
            .expect("failed to serialize OpenApi")
            .into();

        self.router.route(
            path,
            axum::routing::get(serve_spec)
                .layer(Extension(serialized))
        )
    }
}

async fn serve_spec(spec: Extension<Arc<RawValue>>) -> Json<Arc<RawValue>> {
    Json(spec.0)
}
