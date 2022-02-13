//! Run with
//!
//! ```not_rust
//! cargo run -p example-hello-world
//! ```

use axum::{extract::Path, routing::get, Router};
use axum_extra::routing::TypedPath;
use axum_macros::TypedPath;
use serde::Deserialize;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // build our application with a route
    let app = Router::new()
        .route(UsersIndex::PATH, get(|_: Path<UsersIndex>| async {}))
        .route(UsersShow::PATH, get(|_: Path<UsersShow>| async {}))
        .route(UsersEdit::PATH, get(|_: Path<UsersEdit>| async {}));

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Deserialize, TypedPath)]
#[typed_path("/users")]
struct UsersIndex;

// #[derive(Deserialize, TypedPath)]
// #[typed_path("/users/:id/teams/:team_id")]
// struct UsersShow {
//     id: u32,
//     team_id: u32,
// }

// #[derive(Deserialize, TypedPath)]
// #[typed_path("/users/:id/edit")]
// struct UsersEdit(u32);
