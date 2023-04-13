//! Intercepting rejections in a middleware using response extensions.
//!
//! + Easy learning curve: Middlewares and extensions are a well-known feature.
//! + Straightforward: Requires little boilerplate, just the cost of creating a
//!   middleware and checking the response extensions.
//! - Performance: Having this check be done in runtime adds overhead when
//!   compared to solutions like custom extractors.
use axum::{
    extract::{
        rejection::{JsonRejection, QueryRejection},
        Query,
    },
    http::Request,
    middleware::Next,
    response::{IntoResponse, Response},
    Json, Router,
};
use serde_json::{json, Value};

pub fn router() -> Router {
    Router::new()
        .route("/", axum::routing::post(handler))
        .layer(axum::middleware::from_fn(handle_rejection))
}

#[derive(Debug, serde::Deserialize)]
struct QueryTest {
    param: Option<i32>,
}

async fn handler(Query(query): Query<QueryTest>, Json(value): Json<Value>) -> impl IntoResponse {
    dbg!(query.param);
    Json(dbg!(value))
}

async fn handle_rejection<B>(req: Request<B>, next: Next<B>) -> Response {
    let resp = next.run(req).await;

    if let Some(rejection) = resp.extensions().get::<JsonRejection>() {
        let payload = json!({
            "message": rejection.body_text(),
            "type": "json",
            "origin": "response_extension"
        });

        return (resp.status(), axum::Json(payload)).into_response();
    }

    if let Some(rejection) = resp.extensions().get::<QueryRejection>() {
        let payload = json!({
            "message": rejection.body_text(),
            "type": "query",
            "origin": "response_extension"
        });

        return (resp.status(), axum::Json(payload)).into_response();
    }

    resp
}
