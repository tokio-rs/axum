use axum::{extract::Extension, prelude::*, AddExtensionLayer};
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use http::StatusCode;
use std::net::SocketAddr;
use tokio_postgres::NoTls;

#[tokio::main]
async fn main() {
    // setup connection pool
    let manager =
        PostgresConnectionManager::new_from_stringlike("host=localhost user=postgres", NoTls)
            .unwrap();
    let pool = Pool::builder().build(manager).await.unwrap();

    // build our application with some routes
    let app = route("/", get(handler)).layer(AddExtensionLayer::new(pool));

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    hyper::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

type ConnectionPool = Pool<PostgresConnectionManager<NoTls>>;

async fn handler(
    Extension(pool): Extension<ConnectionPool>,
) -> Result<String, (StatusCode, String)> {
    // We cannot get a connection directly via an extractor because
    // `bb8::PooledConnection` contains a reference to the pool and
    // `extract::FromRequest` cannot return types that contain references.
    //
    // So therefore we have to get a connection from the pool manually.
    let conn = pool.get().await.map_err(internal_error)?;

    let row = conn
        .query_one("select 1 + 1", &[])
        .await
        .map_err(internal_error)?;
    let two: i32 = row.try_get(0).map_err(internal_error)?;

    Ok(two.to_string())
}

/// Utility function for mapping any error into a `500 Internal Server Error`
/// response.
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
