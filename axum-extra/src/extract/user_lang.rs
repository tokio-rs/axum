use axum::{async_trait, extract::FromRequestParts};
use http::request::Parts;

/// TBD
#[derive(Debug, Clone)]
pub struct UserLanguage {
    preferred_languages: Vec<String>,
    fallback_language: String,
}

impl UserLanguage {
    /// TBD
    pub fn preferred_language(&self) -> &str {
        self.preferred_languages
            .first()
            .unwrap_or(&self.fallback_language)
    }

    /// TBD
    pub fn preferred_languages(&self) -> &[String] {
        self.preferred_languages.as_slice()
    }

    /// TBD
    pub fn fallback_language(&self) -> &str {
        &self.fallback_language
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for UserLanguage
where
    S: Send + Sync,
{
    type Rejection = ();

    async fn from_request_parts(_parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}
