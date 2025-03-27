//! Example for using associated methods as handlers and middleware.
//!
//! In situations where applications want to encapsulate state (e.g. auth secrets) it
//! can be useful to create routes and middleware directly on specific structs (as opposed to
//! handling everything in a central application state).
//!
//! This example demonstrates how do that using closures.  
//!
//! Run with
//!
//! ```not_rust
//! cargo run -p example-closures
//! ```

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::{from_fn, Next},
    response::IntoResponse,
    routing::get,
    Router,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone)]
struct Username(String);

pub struct Auth {
    database: HashMap<String, String>,
}

impl Auth {
    /// read_username returns the username if it has been previously set by the middleware.
    /// Otherwise it returns 401.
    async fn read_username(self: Arc<Self>, req: Request) -> Result<impl IntoResponse, StatusCode> {
        if let Some(username) = req.extensions().get::<Username>() {
            Ok(format!("hello {}", username.0))
        } else {
            Err(StatusCode::UNAUTHORIZED)
        }
    }

    /// middleware checks if the request has a valid secret and sets the username (if available
    /// in the database) as a request extension).
    async fn middleware(self: Arc<Self>, mut req: Request, next: Next) -> impl IntoResponse {
        if let Some(secret) = req.headers().get("X-Auth-Secret") {
            if let Some(username) = self.database.get(secret.to_str().unwrap()) {
                req.extensions_mut().insert(Username(username.clone()));
            }
        }

        next.run(req).await
    }
}

#[tokio::main]
async fn main() {
    let auth = Auth {
        database: {
            let mut db = HashMap::new();
            db.insert("open sesame".to_string(), "admin".to_string());
            db
        },
    };

    // wrap auth in an Arc smart pointer to share it between threads
    let auth = Arc::new(auth);

    let app = Router::new()
        // route /username to the auth instance behind the smart pointer
        .route(
            "/username",
            get({
                let auth = Arc::clone(&auth);
                |req| async { auth.read_username(req).await }
            }),
        )
        // configure a middleware from the auth instance behind the smart pointer
        .layer(from_fn({
            let auth = Arc::clone(&auth);
            move |req, next| {
                let auth = Arc::clone(&auth);
                async move { auth.middleware(req, next).await }
            }
        }));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}
