use super::{
    sources::{AcceptLanguageSource, PathSource, QuerySource},
    UserLanguageSource,
};
use axum::{async_trait, extract::FromRequestParts};
use http::request::Parts;
use std::convert::Infallible;

/// TBD
#[derive(Debug, Clone)]
pub struct UserLanguage {
    preferred_languages: Vec<String>,
    fallback_language: String,
}

impl UserLanguage {
    /// TBD
    pub fn default_sources<S>() -> Vec<Box<dyn UserLanguageSource<S>>>
    where
        S: Send + Sync,
    {
        vec![
            Box::new(QuerySource::new("lang")),
            Box::new(PathSource::new("lang")),
            Box::new(AcceptLanguageSource),
        ]
    }

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
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let sources = Self::default_sources::<S>();

        let mut preferred_languages = Vec::<String>::new();

        for source in sources {
            let languages = source.languages_from_parts(parts, state).await;
            preferred_languages.extend(languages);
        }

        Ok(UserLanguage {
            preferred_languages,
            fallback_language: "en".to_string(),
        })
    }
}
