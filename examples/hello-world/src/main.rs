//! Run with
//!
//! ```not_rust
//! cargo run -p example-hello-world
//! ```

#![allow(dead_code)]

use axum::{
    extract::{Extension, Json, TypedHeader},
    headers::UserAgent,
    routing::get,
    Router,
};
use axum_macros::FromRequest;
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

#[tokio::main]
async fn main() {
    // build our application with a route
    let app = Router::new().route("/", get(handler));

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler(_: Unit, _: Tuple, _: Named, _: TupleVia, _: NamedVia, _: State) {}

#[derive(FromRequest)]
struct Unit;

#[derive(FromRequest)]
struct Tuple(String);

#[derive(FromRequest)]
struct Named {
    body: String,
}

#[derive(FromRequest)]
struct TupleVia(
    #[from_request(via(Extension))] State,
    #[from_request(via(TypedHeader))] axum::headers::UserAgent,
    #[from_request(via(Json))] Payload,
);

#[derive(FromRequest)]
struct NamedVia {
    state: State,
    #[from_request(via(TypedHeader))]
    user_agent: UserAgent,
    #[from_request(via(Json))]
    body: Payload,
}

#[derive(Clone, FromRequest)]
#[from_request(via(Extension))]
struct State {
    thing: Arc<String>,
}

#[derive(Deserialize)]
struct Payload {
    one: i32,
    two: String,
}
