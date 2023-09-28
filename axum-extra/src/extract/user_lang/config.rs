use std::sync::Arc;

use crate::extract::user_lang::{UserLanguage, UserLanguageSource};

/// Configuration for the [`UserLanguage`] extractor.
/// 
/// By default the [`UserLanguage`] extractor will try to read the
/// languages from the sources returned by [`UserLanguage::default_sources`].
/// 
/// You can override the default behaviour by adding a [`UserLanguageConfig`]
/// extension to your routes.
/// 
/// You can add sources and specify a fallback language.
/// 
/// # Example
/// 
/// ```rust
/// use axum::{routing::get, Extension, Router};
/// use axum_extra::extract::user_lang::{PathSource, QuerySource, UserLanguage};
/// 
/// # fn main() {
/// let app = Router::new()
///     .route("/:lang", get(handler))
///     .layer(Extension(
///         UserLanguage::config()
///             .add_source(QuerySource::new("lang"))
///             .add_source(PathSource::new("lang"))
///             .build(),
///     ));
/// # let _: Router = app;
/// # }
/// # async fn handler() {}
/// ```
///
#[derive(Debug, Clone)]
pub struct UserLanguageConfig {
    pub(crate) fallback_language: String,
    pub(crate) sources: Vec<Arc<dyn UserLanguageSource>>,
}

/// Builder for [`UserLanguageConfig`].
/// 
/// Allows you to declaratively create a [`UserLanguageConfig`].
/// You can create a [`UserLanguageConfigBuilder`] by calling
/// [`UserLanguage::config`].
/// 
/// # Example
/// 
/// ```rust
/// use axum_extra::extract::user_lang::{QuerySource, UserLanguage};
/// 
/// # fn main() {
/// let config = UserLanguage::config()
///     .add_source(QuerySource::new("lang"))
///     .fallback_language("es")
///     .build();
/// # let _ = config;
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct UserLanguageConfigBuilder {
    fallback_language: String,
    sources: Vec<Arc<dyn UserLanguageSource>>,
}

impl UserLanguageConfigBuilder {
    /// Set the fallback language.
    pub fn fallback_language(mut self, fallback_language: impl Into<String>) -> Self {
        self.fallback_language = fallback_language.into();
        self
    }

    /// Add a [`UserLanguageSource`].
    pub fn add_source(mut self, source: impl UserLanguageSource + 'static) -> Self {
        self.sources.push(Arc::new(source));
        self
    }

    /// Create a [`UserLanguageConfig`] from this builder.
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
    /// Returns a builder for [`UserLanguageConfig`].
    pub fn config() -> UserLanguageConfigBuilder {
        UserLanguageConfigBuilder {
            fallback_language: "en".to_string(),
            sources: vec![],
        }
    }
}
