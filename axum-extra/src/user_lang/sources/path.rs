use crate::user_lang::UserLanguageSource;
use axum::{async_trait, extract::Path, RequestPartsExt};
use std::collections::HashMap;

/// TBD
#[derive(Debug, Clone)]
pub struct PathSource {
    /// TBD
    name: String,
}

impl PathSource {
    /// TBD
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait]
impl UserLanguageSource for PathSource {
    async fn languages_from_parts(&self, parts: &mut http::request::Parts) -> Vec<String> {
        let Ok(path) = parts.extract::<Path<HashMap<String, String>>>().await else {
            return vec![];
        };

        let Some(lang) = path.get(self.name.as_str()) else {
            return vec![];
        };

        vec![lang.to_string()]
    }
}
