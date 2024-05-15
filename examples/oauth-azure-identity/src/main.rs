//! Example OAuth (Azure Identity) implementation.
//!
//! 1) Create a new application at <https://learn.microsoft.com/en-us/entra/identity-platform/quickstart-register-app>
//! 2) Gget your CLIENT_ID and CLIENT_SECRET
//! 3) Add a new redirect URI (for this example: `http://127.0.0.1:3000/auth/authorized`)
//! 4) Run with the following (replacing values appropriately):
//! ```not_rust
//! CLIENT_ID=REPLACE_ME CLIENT_SECRET=REPLACE_ME TENANT_ID=REPLACE_ME cargo run -p oauth-azure-identity
//! ```

use anyhow::Context;
use async_session::{serde_json, MemoryStore, Session, SessionStore};
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts, Query, State},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    RequestPartsExt, Router,
};
use axum_extra::{headers, typed_header::TypedHeaderRejectionReason, TypedHeader};
use azure_identity::authorization_code_flow::{self, AuthorizationCodeFlow};
use dotenvy::dotenv;
use http::{
    header::{self, SET_COOKIE},
    request::Parts,
    HeaderMap, StatusCode,
};
use oauth2::{ClientId, ClientSecret, TokenResponse};
use serde::{Deserialize, Serialize};
use std::env;
use url::Url;

static COOKIE_NAME: &str = "SESSION";

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let code_flow = get_code_flow();

    let store = MemoryStore::new();
    let app_state = AppState {
        store,
        auth: MyAuth { code_flow },
    };
    let app = Router::new()
        .route("/", get(index))
        .route("/auth/login", get(login))
        .route("/auth/authorized", get(login_authorized))
        .route("/protected", get(protected))
        .route("/logout", get(logout))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .context("failed to bind TcpListener")
        .unwrap();

    tracing::debug!(
        "listening on {}",
        listener
            .local_addr()
            .context("failed to return local address")
            .unwrap()
    );

    axum::serve(listener, app).await.unwrap();
}

#[derive(Clone)]
struct AppState {
    store: MemoryStore,
    auth: MyAuth,
}

impl FromRef<AppState> for MemoryStore {
    fn from_ref(state: &AppState) -> Self {
        state.store.clone()
    }
}

impl FromRef<AppState> for MyAuth {
    fn from_ref(state: &AppState) -> Self {
        state.auth.clone()
    }
}

struct MyAuth {
    code_flow: AuthorizationCodeFlow,
}

// pkce_code_verifier doesn't support clone so we have to recreate the entire AuthorizationCodeFlow.
impl Clone for MyAuth {
    fn clone(&self) -> Self {
        MyAuth {
            code_flow: AuthorizationCodeFlow {
                authorize_url: self.code_flow.authorize_url.clone(),
                client: self.code_flow.client.clone(),
                csrf_state: oauth2::CsrfToken::new(self.code_flow.csrf_state.secret().clone()),
                pkce_code_verifier: oauth2::PkceCodeVerifier::new(
                    self.code_flow.pkce_code_verifier.secret().clone(),
                ),
            },
        }
    }
}

fn get_code_flow() -> AuthorizationCodeFlow {
    // Environment variables (* = required):
    // *"CLIENT_ID"     "REPLACE_ME";
    // *"CLIENT_SECRET" "REPLACE_ME";
    // *"TENANT_ID"     "REPLACE_ME";
    //  "REDIRECT_URL"  "http://127.0.0.1:3000/auth/authorized";

    let client_id =
        ClientId::new(env::var("CLIENT_ID").expect("Missing CLIENT_ID environment variable."));
    let client_secret = ClientSecret::new(
        env::var("CLIENT_SECRET").expect("Missing CLIENT_SECRET environment variable."),
    );
    let tenant_id = env::var("TENANT_ID").expect("Missing TENANT_ID environment variable.");
    let redirect_url = env::var("REDIRECT_URL")
        .unwrap_or_else(|_| "http://localhost:3000/auth/authorized".to_string());

    authorization_code_flow::start(
        client_id,
        Some(client_secret),
        &tenant_id,
        Url::parse(&redirect_url).unwrap(),
        &["openid", "profile", "email"],
    )
}

// The user data we'll get back from Microsoft Graph.
#[derive(Debug, Serialize, Deserialize)]
struct User {
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "givenName")]
    given_name: String,
    surname: String,
    #[serde(rename = "userPrincipalName")]
    user_principal_name: String,
    id: String,
}

async fn index(user: User) -> impl IntoResponse {
    Html(format!(
        "Hey '{}'. You're logged in!\nYou may now access <a href='/protected'>Protected</a>.\nLog out with <a href='/logout'>Logout</a>.",
        user.display_name))
}

async fn login(State(auth): State<MyAuth>) -> impl IntoResponse {
    Redirect::to(auth.code_flow.authorize_url.as_ref())
}

// Valid user session required. If there is none, redirect to the auth page
async fn protected(user: User) -> impl IntoResponse {
    Html(format!(
        "Welcome to the protected area :)<br />Here's your info:<br />{user:?}"
    ))
}

async fn logout(
    State(store): State<MemoryStore>,
    TypedHeader(cookies): TypedHeader<headers::Cookie>,
) -> Result<impl IntoResponse, AppError> {
    let cookie = cookies
        .get(COOKIE_NAME)
        .context("unexpected error getting cookie name")?;

    let session = match store
        .load_session(cookie.to_string())
        .await
        .context("failed to load session")?
    {
        Some(s) => s,
        // No session active, just redirect
        None => return Ok(Redirect::to("/")),
    };

    store
        .destroy_session(session)
        .await
        .context("failed to destroy session")?;

    Ok(Redirect::to("/"))
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AuthRequest {
    code: String,
    state: String,
}

async fn login_authorized(
    Query(query): Query<AuthRequest>,
    State(store): State<MemoryStore>,
    State(auth): State<MyAuth>,
) -> Result<impl IntoResponse, AppError> {
    let token: oauth2::StandardTokenResponse<
        oauth2::EmptyExtraTokenFields,
        oauth2::basic::BasicTokenType,
    > = auth
        .code_flow
        .exchange(
            azure_core::new_http_client(),
            oauth2::AuthorizationCode::new(query.code.clone()),
        )
        .await
        .unwrap();

    let url = Url::parse("https://graph.microsoft.com/v1.0/me")?;

    let text = reqwest::Client::new()
        .get(url)
        .header(
            "Authorization",
            format!("Bearer {}", token.access_token().secret()),
        )
        .send()
        .await?
        .text()
        .await?;

    let user: User = serde_json::from_str(&text).unwrap();

    println!("\n\nresp {user:?}");

    // Create a new session filled with user data
    let mut session = Session::new();
    session
        .insert("user", user)
        .context("failed in inserting serialized value into session")?;

    // // Store session and get corresponding cookie
    let cookie = store
        .store_session(session)
        .await
        .context("failed to store session")?
        .context("unexpected error retrieving cookie value")?;

    // Build the cookie
    let cookie = format!("{COOKIE_NAME}={cookie}; SameSite=Lax; Path=/");

    // Set cookie
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        cookie.parse().context("failed to parse cookie")?,
    );

    Ok((headers, Redirect::to("/")))
}

struct AuthRedirect;

impl IntoResponse for AuthRedirect {
    fn into_response(self) -> Response {
        Redirect::temporary("/auth/login").into_response()
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for User
where
    MemoryStore: FromRef<S>,
    S: Send + Sync,
{
    // If anything goes wrong or no session is found, redirect to the auth page
    type Rejection = AuthRedirect;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let store = MemoryStore::from_ref(state);

        let cookies = parts
            .extract::<TypedHeader<headers::Cookie>>()
            .await
            .map_err(|e| match *e.name() {
                header::COOKIE => match e.reason() {
                    TypedHeaderRejectionReason::Missing => AuthRedirect,
                    _ => panic!("unexpected error getting Cookie header(s): {e}"),
                },
                _ => panic!("unexpected error getting cookies: {e}"),
            })?;
        let session_cookie = cookies.get(COOKIE_NAME).ok_or(AuthRedirect)?;

        let session = store
            .load_session(session_cookie.to_string())
            .await
            .unwrap()
            .ok_or(AuthRedirect)?;

        let user = session.get::<User>("user").ok_or(AuthRedirect)?;

        Ok(user)
    }
}

// Use anyhow, define error and enable '?'
// For a simplified example of using anyhow in axum check /examples/anyhow-error-response
#[derive(Debug)]
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("Application error: {:#}", self.0);

        (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong").into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
