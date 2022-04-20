#![feature(trace_macros)]

use tower_http::cors::CorsLayer;
use axum::{Json, Server};

/// A Foo
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Foo {
    /// a fooBar
    foo_bar: String,
}

async fn foo_bar(body: Json<Foo>) {

}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    println!("{:#?}", schemars::schema_for!(Foo));

    trace_macros!(true);
    let app = axum_openapi::routing::Router::new()
        .route("/foo/bar", axum_openapi::route! {
            /// Foo
            ///
            /// Bar
            /// Baz
            #[tags("foo", "bar")]
            post: foo_bar
        })
        .spec("A test API serving an OpenAPI spec")
        .serve_at("/openapi.json")
        .layer(CorsLayer::permissive());

    trace_macros!(false);

    Server::bind(&"0.0.0.0:8080".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
