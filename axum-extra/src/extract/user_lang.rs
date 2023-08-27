use axum::{
    async_trait,
    extract::{FromRequestParts, Path, Query},
    RequestPartsExt,
};
use http::request::Parts;
use std::{cmp::Ordering, collections::HashMap, convert::Infallible};

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
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let mut preferred_languages = Vec::<String>::new();

        // First try to get the language from the query string
        if let Ok(query) = parts.extract::<Query<HashMap<String, String>>>().await {
            if let Some(lang) = query.get("lang") {
                preferred_languages.push(lang.to_string());
            }
        };

        // Then try to get the language from the path
        if let Ok(path) = parts.extract::<Path<HashMap<String, String>>>().await {
            if let Some(lang) = path.get("lang") {
                preferred_languages.push(lang.to_string());
            }
        };

        // Then try to get the language from the Accept-Language header
        if let Some(accept_language) = parts.headers.get("Accept-Language") {
            if let Ok(accept_language) = accept_language.to_str() {
                for (lang, _) in parse_quality_values(accept_language) {
                    preferred_languages.push(lang.to_string());
                }
            }
        }

        Ok(UserLanguage {
            preferred_languages,
            fallback_language: "en".to_string(),
        })
    }
}

fn parse_quality_values(values: &str) -> Vec<(&str, f32)> {
    let mut values = values.split(',');
    let mut quality_values = Vec::new();

    while let Some(value) = values.next() {
        let mut value = value.trim().split(';');
        let (value, quality) = (value.next(), value.next());

        let Some(value) = value else {
            // empty quality value entry
            continue;
        };

        let quality = if let Some(quality) = quality.and_then(|q| q.strip_prefix("q=")) {
            quality.parse::<f32>().unwrap_or(0.0)
        } else {
            1.0
        };

        quality_values.push((value, quality));
    }

    quality_values.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    quality_values
}
