use axum::{async_trait, extract::Path, RequestPartsExt};
use std::collections::HashMap;

use crate::extract::user_lang::UserLanguageSource;

/// A source that reads the user language from the request path.
///
/// When creating this source you specify the name of the path
/// segment to read the language from. The routes you want to extract
/// the language from must include a path segment with the configured
/// name for this source to be able to read the language.
///
/// # Example
///
/// The following example will read the language from
/// the path segment `lang_id`. Your routes need to include
/// a `:lang_id` path segment that will contain the language.
///
/// ```rust
/// # use axum::{Router, extract::Extension, routing::get};
/// # use axum_extra::extract::user_lang::{UserLanguage, PathSource};
/// #
/// // The path segment name is `lang_id`.
/// let source = PathSource::new("lang_id");
///
/// // The routes need to include a `:lang_id` path segment.
/// let app = Router::new()
///    .route("/home/:lang_id", get(handler))
///    .layer(
///        Extension(
///            UserLanguage::config()
///                .add_source(source)
///                .build(),
///    ));
///
/// # let _: Router = app;  
/// # async fn handler() {}
/// ```
#[derive(Debug, Clone)]
pub struct PathSource {
    name: String,
}

impl PathSource {
    /// Create a new path source with a given path segment name.
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
