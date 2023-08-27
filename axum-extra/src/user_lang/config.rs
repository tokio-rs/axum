use crate::user_lang::{UserLanguage, UserLanguageSource};
use std::sync::Arc;

/// TBD
#[derive(Debug, Clone)]
pub struct UserLanguageConfig {
    /// TBD
    pub fallback_language: String,

    /// TBD
    pub sources: Vec<Arc<dyn UserLanguageSource>>,
}

/// TBD
#[derive(Debug, Clone)]
pub struct UserLanguageConfigBuilder {
    fallback_language: String,
    sources: Vec<Arc<dyn UserLanguageSource>>,
}

impl UserLanguageConfigBuilder {
    /// TBD
    pub fn fallback_language(mut self, fallback_language: impl Into<String>) -> Self {
        self.fallback_language = fallback_language.into();
        self
    }

    /// TBD
    pub fn add_source(mut self, source: impl UserLanguageSource + 'static) -> Self {
        self.sources.push(Arc::new(source));
        self
    }

    /// TBD
    pub fn build(self) -> UserLanguageConfig {
        UserLanguageConfig {
            fallback_language: self.fallback_language,
            sources: if !self.sources.is_empty() {
                self.sources
            } else {
                UserLanguage::default_sources().clone()
            },
        }
    }
}

impl UserLanguage {
    /// TBD
    pub fn config() -> UserLanguageConfigBuilder {
        UserLanguageConfigBuilder {
            fallback_language: "en".to_string(),
            sources: vec![],
        }
    }
}
