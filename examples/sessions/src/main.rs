//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-sessions
//! ```

use async_session::{MemoryStore, Session, SessionStore as _};
use axum::{
    async_trait,
    extract::{Extension, FromRequest, RequestParts, TypedHeader},
    headers::Cookie,
    http::{
        self,
        header::{HeaderMap, HeaderValue},
        StatusCode,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

const AXUM_SESSION_COOKIE_NAME: &str = "axum_session";

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "example_sessions=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // `MemoryStore` just used as an example. Don't use this in production.
    let store = MemoryStore::new();

    let app = Router::new()
        .route("/", get(handler))
        .layer(Extension(store));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler(user_id: UserIdFromSession) -> impl IntoResponse {
    let (headers, user_id, create_cookie) = match user_id {
        UserIdFromSession::FoundUserId(user_id) => (HeaderMap::new(), user_id, false),
        UserIdFromSession::CreatedFreshUserId(new_user) => {
            let mut headers = HeaderMap::new();
            headers.insert(http::header::SET_COOKIE, new_user.cookie);
            (headers, new_user.user_id, true)
        }
    };

    tracing::debug!("handler: user_id={:?} send_headers={:?}", user_id, headers);

    (
        headers,
        format!(
            "user_id={:?} session_cookie_name={} create_new_session_cookie={}",
            user_id, AXUM_SESSION_COOKIE_NAME, create_cookie
        ),
    )
}

struct FreshUserId {
    pub user_id: UserId,
    pub cookie: HeaderValue,
}

enum UserIdFromSession {
    FoundUserId(UserId),
    CreatedFreshUserId(FreshUserId),
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

        let cookie = Option::<TypedHeader<Cookie>>::from_request(req)
            .await
            .unwrap();

        let session_cookie = cookie
            .as_ref()
            .and_then(|cookie| cookie.get(AXUM_SESSION_COOKIE_NAME));

        // return the new created session cookie for client
        if session_cookie.is_none() {
            let user_id = UserId::new();
            let mut session = Session::new();
            session.insert("user_id", user_id).unwrap();
            let cookie = store.store_session(session).await.unwrap().unwrap();
            return Ok(Self::CreatedFreshUserId(FreshUserId {
                user_id,
                cookie: HeaderValue::from_str(
                    format!("{}={}", AXUM_SESSION_COOKIE_NAME, cookie).as_str(),
                )
                .unwrap(),
            }));
        }

        tracing::debug!(
            "UserIdFromSession: got session cookie from user agent, {}={}",
            AXUM_SESSION_COOKIE_NAME,
            session_cookie.unwrap()
        );
        // continue to decode the session cookie
        let user_id = if let Some(session) = store
            .load_session(session_cookie.unwrap().to_owned())
            .await
            .unwrap()
        {
            if let Some(user_id) = session.get::<UserId>("user_id") {
                tracing::debug!(
                    "UserIdFromSession: session decoded success, user_id={:?}",
                    user_id
                );
                user_id
            } else {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "No `user_id` found in session",
                ));
            }
        } else {
            tracing::debug!(
                "UserIdFromSession: err session not exists in store, {}={}",
                AXUM_SESSION_COOKIE_NAME,
                session_cookie.unwrap()
            );
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
