use super::{
    sources::{AcceptLanguageSource, PathSource, QuerySource},
    UserLanguageConfig, UserLanguageSource,
};
use axum::{async_trait, extract::FromRequestParts, Extension, RequestPartsExt};
use http::request::Parts;
use std::{
    convert::Infallible,
    sync::{Arc, OnceLock},
};

/// The users preferred languages, read from the request.
///
/// This extractor reads the users preferred languages from a
/// configurable list of sources.
/// 
/// By default it will try to read from the following sources:
///   * The query parameter `lang`
///   * The path segment `:lang`
///   * The `Accept-Language` header
/// 
/// # Configuration
/// 
/// To configure the sources see [`UserLanguage::config`] for details.
/// 
/// # Example
/// 
/// ```rust
/// use axum_extra::extract::UserLanguage;
///
/// async fn handler(UserLanguage(lang): UserLanguage) {
///     println!("Preferred languages: {:?}", lang.preferred_languages());
/// }
/// ```
#[derive(Debug, Clone)]
pub struct UserLanguage {
    preferred_languages: Vec<String>,
    fallback_language: String,
}

impl UserLanguage {
    /// TBD
    pub fn default_sources() -> &'static Vec<Arc<dyn UserLanguageSource>> {
        static DEFAULT_SOURCES: OnceLock<Vec<Arc<dyn UserLanguageSource>>> = OnceLock::new();

        DEFAULT_SOURCES.get_or_init(|| {
            vec![
                Arc::new(QuerySource::new("lang")),
                Arc::new(PathSource::new("lang")),
                Arc::new(AcceptLanguageSource),
            ]
        })
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

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let (sources, fallback_language) =
            match parts.extract::<Extension<UserLanguageConfig>>().await {
                Ok(Extension(config)) => (Some(config.sources), Some(config.fallback_language)),
                Err(_) => (None, None),
            };

        let sources = sources.as_ref().unwrap_or(Self::default_sources());
        let fallback_language = fallback_language.unwrap_or_else(|| "en".to_string());

        let mut preferred_languages = Vec::<String>::new();

        for source in sources {
            let languages = source.languages_from_parts(parts).await;
            preferred_languages.extend(languages);
        }

        Ok(UserLanguage {
            preferred_languages,
            fallback_language,
        })
    }
}
