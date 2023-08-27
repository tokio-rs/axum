use std::sync::Arc;

use super::{UserLanguage, UserLanguageSource};

/// TBD
#[derive(Debug, Clone)]
pub struct UserLanguageConfig {
    /// TBD
    pub fallback_language: String,

    /// TBD
    pub sources: Vec<Arc<dyn UserLanguageSource>>,
}

#[derive(Debug, Clone)]
pub struct UserLanguageConfigBuilder {
    fallback_language: String,
    sources: Vec<Arc<dyn UserLanguageSource>>,
}

impl UserLanguageConfigBuilder {
    pub fn fallback_language(mut self, fallback_language: impl Into<String>) -> Self {
        self.fallback_language = fallback_language.into();
        self
    }

    pub fn add_source(mut self, source: impl UserLanguageSource + 'static) -> Self {
        self.sources.push(Arc::new(source));
        self
    }

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
