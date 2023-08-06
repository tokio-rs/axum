//! Example of application streaming large csv from postgres using <https://github.com/launchbadge/sqlx>
//!
//! Run with
//!
//! ```not_rust
//! cargo run -p example-sqlx-postgres-stream
//! ```
//!
//! Test with curl:
//!
//! ```not_rust
//! curl 127.0.0.1:3000
//! ```

use std::{net::SocketAddr, time::Duration};

use axum::{
    body::StreamBody, extract::State, http::header, http::StatusCode, response::IntoResponse,
    routing::get, Router,
};
use futures::TryStreamExt;
use sqlx::postgres::{PgPool, PgPoolOptions};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::self_ref_stream::SelfRefStream;

mod self_ref_stream;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_tokio_postgres=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_connection_str = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost".to_string());

    // setup connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect(&db_connection_str)
        .await
        .expect("can't connect to database");

    // build our application with some routes
    let app = Router::new()
        .route("/", get(using_self_ref_stream))
        .with_state(pool);

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// we can extract the connection pool with `State`
async fn using_self_ref_stream(State(pool): State<PgPool>) -> impl IntoResponse {
    // sqlx requires borrowed pool, so we need to wrap original stream with SelfRefStream
    let body = SelfRefStream::build(pool, |pool_ref| {
        sqlx::query_scalar::<_, String>(
            r#"SELECT col1 FROM (VALUES ('hello'), ('from'), ('postgres')) AS q (col1)"#,
        )
        .fetch(pool_ref)
    })
    .map_ok(|s| s + "\n");

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain")],
        StreamBody::new(body),
    )
}
