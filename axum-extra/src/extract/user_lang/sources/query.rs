use axum::{async_trait, RequestPartsExt};
use std::collections::HashMap;

use crate::extract::{Query, user_lang::UserLanguageSource};

/// A [`UserLanguageSource`] that reads the language from a field in the
/// query string.
///
/// When creating this source you specify the name of the query
/// field to read the language from. You can add multiple `QuerySource`
/// instances to read from different fields.
///
/// # Example
///
/// The following example will read the language from
/// the query field `lang_id`.
///
/// ```rust
/// # use axum::{Router, extract::Extension, routing::get};
/// # use axum_extra::extract::user_lang::{UserLanguage, QuerySource};
/// #
/// // The query field name is `lang_id`.
/// let source = QuerySource::new("lang_id");
///
/// let app = Router::new()
///    .route("/home", get(handler))
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
pub struct QuerySource {
    name: String,
}

impl QuerySource {
    /// Create a new query source with a given query field name.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait]
impl UserLanguageSource for QuerySource {
    async fn languages_from_parts(&self, parts: &mut http::request::Parts) -> Vec<String> {
        let Ok(query) = parts.extract::<Query<HashMap<String, String>>>().await else {
            return vec![];
        };

        let Some(lang) = query.get(self.name.as_str()) else {
            return vec![];
        };

        vec![lang.to_owned()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{Request, Uri};

    #[tokio::test]
    async fn reads_language_from_query() {
        let source = QuerySource::new("lang");

        let request: Request<()> = Request::builder()
            .uri(Uri::builder().path_and_query("/?lang=de").build().unwrap())
            .body(())
            .unwrap();

        let (mut parts, _) = request.into_parts();

        let languages = source.languages_from_parts(&mut parts).await;

        assert_eq!(languages, vec!["de".to_owned()]);
    }
}
