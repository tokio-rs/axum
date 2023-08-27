use axum::async_trait;
use http::request::Parts;
use std::fmt::Debug;

/// TBD
#[async_trait]
pub trait UserLanguageSource: Send + Sync + Debug {
    /// TBD
    async fn languages_from_parts(&self, parts: &mut Parts) -> Vec<String>;
}
