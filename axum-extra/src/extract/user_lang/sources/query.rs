use crate::extract::UserLanguageSource;
use axum::{async_trait, extract::Query, RequestPartsExt};
use std::collections::HashMap;

/// TBD
#[derive(Debug, Clone)]
pub struct QuerySource {
    /// TBD
    name: String,
}

impl QuerySource {
    /// TBD
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait]
impl<S> UserLanguageSource<S> for QuerySource {
    async fn languages_from_parts(
        &self,
        parts: &mut http::request::Parts,
        _state: &S,
    ) -> Vec<String> {
        let Ok(query) = parts.extract::<Query<HashMap<String, String>>>().await else {
            return vec![];
        };

        let Some(lang) = query.get(self.name.as_str()) else {
            return vec![];
        };

        vec![lang.to_string()]
    }
}
