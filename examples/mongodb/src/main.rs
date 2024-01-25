//! Run with
//!
//! ```not_rust
//! cargo run -p example-mongodb
//! ```

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use mongodb::{bson::doc, Client, Collection};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // connecting to mongodb
    let db_connection_str = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "mongodb://admin:password@127.0.0.1:27017/?authSource=admin".to_string()
    });
    let client = Client::with_uri_str(db_connection_str).await.unwrap();

    // pinging the database
    client
        .database("axum-mongo")
        .run_command(doc! { "ping": 1 }, None)
        .await
        .unwrap();
    println!("Pinged your database. Successfully connected to MongoDB!");

    // logging middleware
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_mongo=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app(client)).await.unwrap();
}

// defining routes and state
fn app(client: Client) -> Router {
    let collection: Collection<Member> = client.database("axum-mongo").collection("members");

    Router::new()
        .route("/create", post(create_member))
        .route("/read/:id", get(read_member))
        .route("/update", put(update_member))
        .route("/delete/:id", delete(delete_member))
        .layer(TraceLayer::new_for_http())
        .with_state(collection)
}

// handler to create a new member
async fn create_member(
    State(db): State<Collection<Member>>,
    Json(input): Json<Member>,
) -> impl IntoResponse {
    let result = db.insert_one(input, None).await.unwrap();
    println!("{:?}", result);

    (StatusCode::CREATED, Json(result))
}

// handler to read an existing member
async fn read_member(
    State(db): State<Collection<Member>>,
    Path(id): Path<u32>,
) -> impl IntoResponse {
    let result = db.find_one(doc! { "_id": id }, None).await.unwrap();
    println!("{:?}", result);

    if result.is_none() {
        return (StatusCode::NOT_FOUND, Json(result));
    }

    (StatusCode::OK, Json(result))
}

// handler to update an existing member
async fn update_member(
    State(db): State<Collection<Member>>,
    Json(input): Json<Member>,
) -> impl IntoResponse {
    let result = db
        .replace_one(doc! { "_id": input._id }, input, None)
        .await
        .unwrap();
    println!("{:?}", result);

    (StatusCode::OK, Json(result))
}

// handler to delete an existing member
async fn delete_member(
    State(db): State<Collection<Member>>,
    Path(id): Path<u32>,
) -> impl IntoResponse {
    let result = db.delete_one(doc! { "_id": id }, None).await.unwrap();
    println!("{:?}", result);

    (StatusCode::OK, Json(result))
}

// defining Member type
#[derive(Debug, Deserialize, Serialize)]
struct Member {
    _id: u32,
    name: String,
    active: bool,
}
