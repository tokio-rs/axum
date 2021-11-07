//! Run with
//!
//! ```not_rust
//! cargo run -p example-sessions
//! ```

use async_session::{MemoryStore, Session, SessionStore as _};
use axum::{
    async_trait,
    extract::{Extension, FromRequest, RequestParts},
    http::{
        self,
        header::{HeaderMap, HeaderValue},
        StatusCode,
    },
    response::IntoResponse,
    routing::get,
    AddExtensionLayer, Router,
};
use cookie::Cookie;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use uuid::Uuid;

lazy_static! {
    static ref SESSION_ID: String = Uuid::new_v4().to_string();
}

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "example_sessions=debug")
    }
    tracing_subscriber::fmt::init();

    // `MemoryStore` just used as an example. Don't use this in production.
    let store = MemoryStore::new();

    let app = Router::new()
        .route("/", get(handler))
        .layer(AddExtensionLayer::new(store));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler(user_id: UserIdFromSession) -> impl IntoResponse {
    let (headers, user_id) = match user_id {
        UserIdFromSession::FoundUserId(user_id) => (HeaderMap::new(), user_id),
        UserIdFromSession::CreatedFreshUserId { user_id, cookie } => {
            let mut headers = HeaderMap::new();
            headers.insert(http::header::SET_COOKIE, cookie);
            (headers, user_id)
        }
    };

    dbg!(user_id);

    headers
}

enum UserIdFromSession {
    FoundUserId(UserId),
    CreatedFreshUserId {
        user_id: UserId,
        cookie: HeaderValue,
    },
}

#[async_trait]
impl<B> FromRequest<B> for UserIdFromSession
where
    B: Send,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let Extension(store) = Extension::<MemoryStore>::from_request(req)
            .await
            .expect("`MemoryStore` extension missing");

        let headers = req.headers().expect("other extractor taken headers");

        let cookie = headers
            .get(http::header::COOKIE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string())
            .unwrap_or("".to_string());

        let cookie_map = parse_cookie(cookie);

        if !cookie_map.contains_key(&*SESSION_ID) {
            let user_id = UserId::new();
            let mut session = Session::new();
            session.insert("user_id", user_id).unwrap();
            let cookie = store.store_session(session).await.unwrap().unwrap();

            let cookie = Cookie::build(&*SESSION_ID, &cookie).finish().to_string();
            return Ok(Self::CreatedFreshUserId {
                user_id,
                cookie: cookie.parse().unwrap(),
            });
        }

        let cookie = cookie_map.get(&*SESSION_ID).unwrap();
        let user_id = if let Some(session) = store.load_session(cookie.to_string()).await.unwrap() {
            if let Some(user_id) = session.get::<UserId>("user_id") {
                user_id
            } else {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "No `user_id` found in session",
                ));
            }
        } else {
            return Err((StatusCode::BAD_REQUEST, "No session found for cookie"));
        };

        Ok(Self::FoundUserId(user_id))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
struct UserId(Uuid);

impl UserId {
    fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

fn parse_cookie(cookie: String) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let v: Vec<&str> = cookie.split(";").collect();
    for cookie in &v {
        let kv: Vec<&str> = cookie.split("=").collect();
        if kv.len() < 2 {
            return HashMap::new();
        }
        let key = kv[0].trim_start();
        let value = kv[1].trim_end();
        map.insert(key.to_string(), value.to_string());
    }
    map
}
