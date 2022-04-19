use tower_http::cors::CorsLayer;
use axum::{Json, Server};

/// A Foo
#[derive(serde::Deserialize, axum_openapi::JsonBody)]
#[serde(rename_all = "camelCase")]
struct Foo {
    /// a fooBar
    foo_bar: String,
}


/// Foo
///
/// Bar
/// Baz
#[axum_openapi::route]
async fn foo_bar(body: Json<Foo>) {

}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let app = axum_openapi::routing::Router::new()
        .route("/foo/bar", axum_openapi::routing::post(foo_bar))
        .spec("A test API serving an OpenAPI spec")
        .serve_at("/openapi.json")
        .layer(CorsLayer::permissive());

    Server::bind(&"0.0.0.0:8080".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
