//! Run with
//!
//! ```not_rust
//! cargo run -p example-sqlx-sqlite
//! ```
//!
//! Test with curl:
//!
//! ```not_rust
//! curl 127.0.0.1:3000
//! ```

use axum::{
    async_trait,
    extract::{Extension, FromRequest, RequestParts},
    http::StatusCode,
    routing::get,
    AddExtensionLayer, Router,
};
use sqlx::any::Any;
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::SqlitePool;

use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "example_tokio_postgres=debug")
    }
    tracing_subscriber::fmt::init();

    let uri = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://./target/db.sqlite3".to_string());

    if !Any::database_exists(&uri).await.unwrap() {
        Any::create_database(&uri).await.unwrap();
        tracing::info!("created sqlite database at {}", uri);
    }

    let pool = SqlitePool::connect(&uri).await.unwrap();

    // embeds migrations inside the binary
    sqlx::migrate!().run(&pool).await.unwrap();

    // build our application with some routes
    let app = Router::new()
        .route(
            "/",
            get(using_connection_pool_extractor).post(using_connection_extractor),
        )
        .layer(AddExtensionLayer::new(pool));

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// we can exact the connection pool with `Extension`
async fn using_connection_pool_extractor(
    Extension(pool): Extension<SqlitePool>,
) -> Result<String, (StatusCode, String)> {
    let (result,) = sqlx::query_as("select username from users where id = $1")
        .bind(1_u32)
        .fetch_one(&pool)
        .await
        .map_err(internal_error)?;

    Ok(result)
}

// we can also write a custom extractor that grabs a connection from the pool
// which setup is appropriate depends on your application
struct DatabaseConnection(sqlx::pool::PoolConnection<sqlx::Sqlite>);

#[async_trait]
impl<B> FromRequest<B> for DatabaseConnection
where
    B: Send,
{
    type Rejection = (StatusCode, String);

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let Extension(pool) = Extension::<SqlitePool>::from_request(req)
            .await
            .map_err(internal_error)?;

        let conn = pool.acquire().await.map_err(internal_error)?;

        Ok(Self(conn))
    }
}

async fn using_connection_extractor(
    DatabaseConnection(conn): DatabaseConnection,
) -> Result<String, (StatusCode, String)> {
    let mut conn = conn;
    let row: (String,) = sqlx::query_as("select username from users where id = $1")
        .bind(1_u32)
        .fetch_one(&mut conn)
        .await
        .map_err(internal_error)?;

    Ok(row.0)
}

/// Utility function for mapping any error into a `500 Internal Server Error`
/// response.
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
