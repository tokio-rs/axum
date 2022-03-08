use axum::{
    extract::Extension,
    http::uri::Uri,
    response::IntoResponse,
    routing::{get, post},
};
use sqlx::PgPool;
use tera::Tera;
use tower_cookies::{Cookie, CookieManagerLayer};

use login::*;
use todos::*;

mod login;
mod todos;

#[tokio::main]
async fn main() {
    // Getting Database URL from environment variables if exists or setting default URL.
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1597@localhost:3000/postgres".to_string());
    // Creating connection for SQLX with our Database.
    let pool = PgPool::connect(&db_url).await.unwrap();
    // Creating an instance of Tera Template Engine.
    let tera = Tera::new("templates/*.html").unwrap();

    // Setting up handlers and Layers, which allow for us use SQLX, Tera and tower_cookies.
    let app = axum::Router::new()
        .route("/", get(list_todos))
        .route("/login", get(login_into_account))
        .route("/register", get(register_page).post(register))
        .route("/logout", get(logout))
        .route("/new", get(editing_new_todo).post(create_todo))
        .route("/edit/:id", get(edit_todo).post(update_todo))
        .route("/:id", post(delete_todo).get(get_description))
        .route("/reset", get(delete_all_todos).post(delete_all_done_todos))
        .layer(Extension(pool))
        .layer(Extension(tera))
        .layer(CookieManagerLayer::new());

    // Setting localhost URL with port 8000.
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], 8000));
    axum::Server::bind(&address)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
