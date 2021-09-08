//! Example JWT authorization/authentication.
//!
//! Run with
//!
//! ```not_rust
//! JWT_SECRET=secret cargo run -p example-jwt
//! ```

use axum::{
    body::{Bytes, Full},
    extract::TypedHeader,
    handler::{get, post},
    http::{Response, StatusCode},
    response::IntoResponse,
    Json, Router,
};
use headers::{authorization::Bearer, Authorization};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{convert::Infallible, fmt::Display, net::SocketAddr};

// Quick instructions
//
// - get an authorization token:
//
// curl -s \
//     -w '\n' \
//     -H 'Content-Type: application/json' \
//     -d '{"client_id":"foo","client_secret":"bar"}' \
//     http://localhost:3000/auth
//
// - visit the protected area using the token for authentication
//
// curl -s \
//     -w '\n' \
//     -H 'Content-Type: application/json' \
//     -H 'Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJiQGIuY29tIiwiY29tcGFueSI6IkFDTUUiLCJleHAiOjEwMDAwMDAwMDAwfQ.M3LAZmrzUkXDC1q5mSzFAs_kJrwuKz3jOoDmjJ0G4gM' \
//     http://localhost:3000/protected
//
// - try to visit the protected area using an invalid token
//
// curl -s \
//     -w '\n' \
//     -H 'Content-Type: application/json' \
//     -H 'Authorization: Bearer blahblahblah' \
//     http://localhost:3000/protected

static KEYS: Lazy<Keys> = Lazy::new(|| {
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    Keys::new(secret.as_bytes())
});

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "example_jwt=debug")
    }
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/protected", get(protected))
        .route("/auth", post(auth));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn protected(
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
) -> Result<String, AuthError> {
    // Extract the authorization header value (i.e the token)
    let token = authorization.0.token();

    // Authenticate the user and returns his data
    let validation = Validation {
        sub: Some("b@b.com".to_string()),
        ..Validation::default()
    };
    let token_data =
        decode::<Claims>(token, &KEYS.decoded, &validation).map_err(|_| AuthError::InvalidToken)?;

    // Send the protected data to the user
    Ok(format!(
        "Welcome to the protected area :)\nYour data:\n{}",
        token_data.claims
    ))
}

async fn auth(Json(payload): Json<AuthPayload>) -> Result<Json<AuthBody>, AuthError> {
    // Check if the user sent the credentials
    if payload.client_id.is_empty() || payload.client_secret.is_empty() {
        return Err(AuthError::MissingCredentials);
    }
    // Here you can check the user credentials from a database
    if payload.client_id != "foo" || payload.client_secret != "bar" {
        return Err(AuthError::WrongCredentials);
    }

    // Create the authorization token
    let claims = Claims {
        sub: "b@b.com".to_owned(),
        company: "ACME".to_owned(),
        exp: 10000000000,
    };
    let token =
        encode(&Header::default(), &claims, &KEYS.encoded).map_err(|_| AuthError::TokenCreation)?;

    // Send the authorization token
    Ok(Json(AuthBody::new(token)))
}

impl Display for Claims {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Email: {}\nCompany: {}", self.sub, self.company)
    }
}

impl AuthBody {
    fn new(access_token: String) -> Self {
        Self {
            access_token,
            token_type: "Bearer".to_string(),
        }
    }
}

impl IntoResponse for AuthError {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let (status, error_message) = match self {
            AuthError::WrongCredentials => (StatusCode::UNAUTHORIZED, "Wrong credentials"),
            AuthError::MissingCredentials => (StatusCode::BAD_REQUEST, "Missing credentials"),
            AuthError::TokenCreation => (StatusCode::INTERNAL_SERVER_ERROR, "Token creation error"),
            AuthError::InvalidToken => (StatusCode::BAD_REQUEST, "Invalid token"),
        };
        let body = Json(json!({
            "error": error_message,
        }));
        (status, body).into_response()
    }
}

#[derive(Debug)]
struct Keys {
    encoded: EncodingKey,
    decoded: DecodingKey<'static>,
}

impl Keys {
    fn new(secret: &[u8]) -> Self {
        Self {
            encoded: EncodingKey::from_secret(secret),
            decoded: DecodingKey::from_secret(secret).into_static(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    company: String,
    exp: usize,
}

#[derive(Debug, Serialize)]
struct AuthBody {
    access_token: String,
    token_type: String,
}

#[derive(Debug, Deserialize)]
struct AuthPayload {
    client_id: String,
    client_secret: String,
}

#[derive(Debug)]
enum AuthError {
    WrongCredentials,
    MissingCredentials,
    TokenCreation,
    InvalidToken,
}
