//! Example OAuth (Discord) implementation.
//!
//! 1) Create a new application at <https://discord.com/developers/applications>
//! 2) Visit the OAuth2 tab to get your CLIENT_ID and CLIENT_SECRET
//! 3) Add a new redirect URI (for this example: `http://127.0.0.1:3000/auth/authorized`)
//! 4) Run with the following (replacing values appropriately):
//! ```not_rust
//! CLIENT_ID=REPLACE_ME CLIENT_SECRET=REPLACE_ME cargo run -p example-oauth
//! ```

use anyhow::{anyhow, Context, Result};
use axum::{
    extract::{Query, State},
    http::{header::SET_COOKIE, HeaderMap},
    response::{IntoResponse, Redirect, Response},
    routing::get,
    Router,
};
use axum_extra::{headers, TypedHeader};
use http::StatusCode;
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::{env, str::FromStr, time::Duration};
use tower_sessions::{
    session::{Id, Record},
    CachingSessionStore, SessionStore,
};
use tower_sessions_moka_store::MokaStore;
use tower_sessions_rusqlite_store::{tokio_rusqlite, RusqliteStore};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// The cookie to store the session id for user information.
const SESSION_COOKIE: &str = "info";
const SESSION_DATA_KEY: &str = "data";

/// The time a user can be logged in before needing to re-authenticate.
/// (24 * 3600 seconds = 1 day)
const SESSION_EXPIRATION: Duration = Duration::from_secs(24 * 3600);

/// The cookie used to pass the CSRF token through discord auth.
const OAUTH_CSRF_COOKIE: &str = "SESSION";
const CSRF_TOKEN: &str = "csrf_token";

/// The time we allow to re-direct to an authentication service then back to our
/// application (3600 seconds = 1 hour).
const OAUTH_SESSION_EXPIRATION: Duration = Duration::from_secs(3600);

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Create session backing SQL database.
    // While some monolithic application may use a local file in production to back their database,
    // it is common that you would setup a connection to a remote service. You may need to change
    // `conn` to suit your use-case.
    let file = tempfile::NamedTempFile::new().unwrap();
    let conn = tokio_rusqlite::Connection::open(file.path()).await.unwrap();
    let sql_store = RusqliteStore::new(conn);

    // Create session in-memory cache.
    let moka_store = MokaStore::new(Some(100));
    let store = CachingSessionStore::new(moka_store, sql_store);

    // Create app state.
    let oauth_client = oauth_client().unwrap();
    let app_state = AppState {
        oauth_client,
        store,
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/auth/discord", get(discord_auth))
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
    oauth_client: BasicClient,
    store: CachingSessionStore<MokaStore, RusqliteStore>,
}

fn oauth_client() -> Result<BasicClient, AppError> {
    // Environment variables (* = required):
    // *"CLIENT_ID"     "REPLACE_ME";
    // *"CLIENT_SECRET" "REPLACE_ME";
    //  "REDIRECT_URL"  "http://127.0.0.1:3000/auth/authorized";
    //  "AUTH_URL"      "https://discord.com/api/oauth2/authorize?response_type=code";
    //  "TOKEN_URL"     "https://discord.com/api/oauth2/token";

    let client_id = env::var("CLIENT_ID").context("Missing CLIENT_ID!")?;
    let client_secret = env::var("CLIENT_SECRET").context("Missing CLIENT_SECRET!")?;
    let redirect_url = env::var("REDIRECT_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3000/auth/authorized".to_string());

    let auth_url = env::var("AUTH_URL").unwrap_or_else(|_| {
        "https://discord.com/api/oauth2/authorize?response_type=code".to_string()
    });

    let token_url = env::var("TOKEN_URL")
        .unwrap_or_else(|_| "https://discord.com/api/oauth2/token".to_string());

    Ok(BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        AuthUrl::new(auth_url).context("failed to create new authorization server URL")?,
        Some(TokenUrl::new(token_url).context("failed to create new token endpoint URL")?),
    )
    .set_redirect_uri(
        RedirectUrl::new(redirect_url).context("failed to create new redirection URL")?,
    ))
}

// The user data we'll get back from Discord.
// https://discord.com/developers/docs/resources/user#user-object-user-structure
#[derive(Debug, Serialize, Deserialize)]
struct User {
    id: String,
    avatar: Option<String>,
    username: String,
    discriminator: String,
}

// Session is optional
async fn index(
    State(AppState { store, .. }): State<AppState>,
    TypedHeader(cookies): TypedHeader<headers::Cookie>,
) -> Result<impl IntoResponse, AppError> {
    let session_id = Id::from_str(
        cookies
            .get(SESSION_COOKIE)
            .context("missing session cookie")?,
    )
    .unwrap();
    let session = store.load(&session_id).await.unwrap().unwrap();
    let user = session
        .data
        .get(SESSION_DATA_KEY)
        .map(|v| serde_json::from_value::<User>(v.clone()).unwrap());
    Ok(match user {
        Some(u) => format!(
            "Hey {}! You're logged in!\nYou may now access `/protected`.\nLog out with `/logout`.",
            u.username
        ),
        None => "You're not logged in.\nVisit `/auth/discord` to do so.".to_string(),
    })
}

async fn discord_auth(
    State(AppState {
        oauth_client,
        store,
    }): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let (auth_url, csrf_token) = oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("identify".to_string()))
        .url();

    // Store the token in the session and retrieve the session cookie.
    let session_id = Id(i128::from_le_bytes(uuid::Uuid::new_v4().to_bytes_le()));
    store
        .create(&mut Record {
            id: session_id,
            data: [(
                CSRF_TOKEN.to_string(),
                serde_json::to_value(csrf_token).unwrap(),
            )]
            .into(),
            expiry_date: time::OffsetDateTime::now_utc() + OAUTH_SESSION_EXPIRATION,
        })
        .await
        .context("failed in inserting CSRF token into session")?;

    // Attach the session cookie to the response header
    let cookie =
        format!("{OAUTH_CSRF_COOKIE}={session_id}; SameSite=Lax; HttpOnly; Secure; Path=/");
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        cookie.parse().context("failed to parse cookie")?,
    );

    Ok((headers, Redirect::to(auth_url.as_ref())))
}

// Valid user session required. If there is none, redirect to the auth page
async fn protected(
    State(AppState { store, .. }): State<AppState>,
    TypedHeader(cookies): TypedHeader<headers::Cookie>,
) -> Result<impl IntoResponse, AppError> {
    let session_id = Id::from_str(
        cookies
            .get(SESSION_COOKIE)
            .context("missing session cookie")?,
    )
    .unwrap();
    let session = store
        .load(&session_id)
        .await
        .unwrap()
        .context("missing session")?;
    let user = serde_json::from_value::<User>(session.data.get(SESSION_DATA_KEY).unwrap().clone())
        .unwrap();
    Ok(format!(
        "Welcome to the protected area :)\nHere's your info:\n{user:?}"
    ))
}

async fn logout(
    State(AppState { store, .. }): State<AppState>,
    TypedHeader(cookies): TypedHeader<headers::Cookie>,
) -> Result<impl IntoResponse, AppError> {
    let session_id = Id::from_str(
        cookies
            .get(SESSION_COOKIE)
            .context("missing session cookie")?,
    )
    .unwrap();
    store
        .delete(&session_id)
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

async fn csrf_token_validation_workflow(
    auth_request: &AuthRequest,
    store: &CachingSessionStore<MokaStore, RusqliteStore>,
    oauth_session_id: Id,
) -> Result<(), AppError> {
    let oauth_session = store.load(&oauth_session_id).await.unwrap().unwrap();

    // Extract the CSRF token from the session
    let csrf_token_serialized = oauth_session
        .data
        .get(CSRF_TOKEN)
        .context("failed to get value from session")?;
    let csrf_token = serde_json::from_value::<CsrfToken>(csrf_token_serialized.clone())
        .context("CSRF token not found in session")?
        .to_owned();

    // Cleanup the CSRF token session
    store
        .delete(&oauth_session_id)
        .await
        .context("Failed to destroy old session")?;

    // Validate CSRF token is the same as the one in the auth request
    if *csrf_token.secret() != auth_request.state {
        return Err(anyhow!("CSRF token mismatch").into());
    }

    Ok(())
}

async fn login_authorized(
    State(AppState {
        oauth_client,
        store,
    }): State<AppState>,
    TypedHeader(cookies): TypedHeader<headers::Cookie>,
    Query(query): Query<AuthRequest>,
) -> Result<impl IntoResponse, AppError> {
    let oauth_session_id = Id::from_str(
        cookies
            .get(OAUTH_CSRF_COOKIE)
            .context("missing session cookie")?,
    )
    .unwrap();
    csrf_token_validation_workflow(&query, &store, oauth_session_id).await?;

    // Get an auth token
    let token = oauth_client
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        .request_async(async_http_client)
        .await
        .context("failed in sending request request to authorization server")?;

    // Fetch user data from discord
    let client = reqwest::Client::new();
    let user_data: User = client
        // https://discord.com/developers/docs/resources/user#get-current-user
        .get("https://discordapp.com/api/users/@me")
        .bearer_auth(token.access_token().secret())
        .send()
        .await
        .context("failed in sending request to target Url")?
        .json::<User>()
        .await
        .context("failed to deserialize response as JSON")?;

    // Create a new session filled with user data
    let session_id = Id(i128::from_le_bytes(uuid::Uuid::new_v4().to_bytes_le()));
    store
        .create(&mut Record {
            id: session_id,
            data: [(
                SESSION_DATA_KEY.to_string(),
                serde_json::to_value(user_data).unwrap(),
            )]
            .into(),
            expiry_date: time::OffsetDateTime::now_utc() + SESSION_EXPIRATION,
        })
        .await
        .context("failed in inserting serialized value into session")?;

    // Store session and get corresponding cookie.
    let cookie = format!("{SESSION_COOKIE}={session_id}; SameSite=Lax; HttpOnly; Secure; Path=/");

    // Set cookie
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        cookie.parse().context("failed to parse cookie")?,
    );

    Ok((headers, Redirect::to("/")))
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
