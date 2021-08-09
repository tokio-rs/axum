//! Run with
//!
//! ```not_rust
//! cargo run --example open_api --features open_api
//! ```

#![allow(dead_code, unused_imports)]

use axum::{
    extract::{Extension, Path, Query},
    open_api::{self, *},
    prelude::*,
    response::{Html, IntoResponse},
    routing::{nest, BoxRoute},
    AddExtensionLayer, Json,
};
use openapiv3::OpenAPI;
use serde::Deserialize;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let api = route(
        "/api/users",
        get(get_users
            .operation_id("get_users_paginated")
            .map_operation(|mut operation| {
                operation.description = Some("Get all the users as a paginated list".to_string());
                operation
            }))
        .post((|| async {}).operation_id("create_user")),
    )
    .route(
        // TODO(david): path parameters
        "/api/users/:id",
        delete(delete_user.operation_id("delete_user")),
    );

    let normal_routes = nest("/foo", nested_routes());
    let app = api.or(normal_routes);

    let open_api = open_api::to_open_api(&app);

    {
        let yaml = serde_yaml::to_string(&open_api).unwrap();
        println!("{}", yaml);
    }

    let app = app
        .route(
            "/openapi.json",
            get(|Extension(open_api): Extension<Arc<OpenAPI>>| async move { Json(open_api) }),
        )
        .layer(AddExtensionLayer::new(Arc::new(open_api)));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn get_users(_: Query<Pagination>) -> Html<&'static str> {
    Html("users")
}

async fn delete_user(Path(id): Path<u32>) -> &'static str {
    "deleting user..."
}

fn nested_routes() -> WithPaths<BoxRoute<Body>> {
    route("/bar", get(|| async {})).boxed_with_open_api()
}

#[derive(Deserialize)]
struct Pagination {
    offset: Option<usize>,
    limit: Option<usize>,
}

// we're gonna need #[derive(ToQueryParameter)] for this :/
impl ToQueryParameter for Pagination {
    fn to_query_parameter() -> open_api::Query {
        use openapiv3::*;

        let mut obj = ObjectType::default();

        obj.properties.insert(
            "offset".to_string(),
            ReferenceOr::Item(Box::new(Option::<usize>::to_schema())),
        );

        obj.properties.insert(
            "limit".to_string(),
            ReferenceOr::Item(Box::new(Option::<usize>::to_schema())),
        );

        let schema = Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Type(Type::Object(obj)),
        };

        let parameter_data = ParameterData {
            name: "Pagination".to_string(),
            description: None,
            required: false,
            deprecated: Some(false),
            format: ParameterSchemaOrContent::Schema(ReferenceOr::Item(schema)),
            example: None,
            examples: Default::default(),
            explode: None,
            extensions: Default::default(),
        };

        open_api::Query {
            parameter_data,
            allow_reserved: false,
            style: QueryStyle::default(),
            allow_empty_value: Some(true),
        }
    }
}
