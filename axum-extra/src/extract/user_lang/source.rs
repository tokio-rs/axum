use axum::async_trait;
use http::request::Parts;

/// TBD
#[async_trait]
pub trait UserLanguageSource<S>: Send + Sync {
    /// TBD
    async fn languages_from_parts(&self, parts: &mut Parts, state: &S) -> Vec<String>;
}
