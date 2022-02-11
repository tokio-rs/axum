//! Run with
//!
//! ```not_rust
//! cargo run -p example-hello-world
//! ```

#![allow(dead_code)]

use axum::{response::Html, routing::get, Router};
use axum_macros::Uri;
use serde::Deserialize;
use std::net::SocketAddr;

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

async fn handler(_: UsersShow) -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

#[derive(Deserialize, Uri)]
#[uri("/users")]
struct UsersIndex;

#[derive(Deserialize, Uri)]
#[uri("/users/:id/teams/:team_id")]
struct UsersShow {
    id: u32,
    team_id: u32,
}

// #[derive(serde::Deserialize)]
// #[route("/users/:id/edit")]
// struct UsersEdit(u32);
