use super::{
    sources::{AcceptLanguageSource, PathSource},
    Config, UserLanguageSource,
};
use axum::{async_trait, extract::FromRequestParts, Extension, RequestPartsExt};
use http::request::Parts;
use std::{
    convert::Infallible,
    sync::{Arc, OnceLock},
};

#[cfg(feature = "query")]
use super::sources::QuerySource;

/// The users preferred languages, read from the request.
///
/// This extractor reads the users preferred languages from a
/// configurable list of sources.
///
/// By default it will try to read from the following sources:
///  * The query parameter `lang`
///  * The path segment `:lang`
///  * The `Accept-Language` header
///
/// This extractor never fails. If no language could be read from the request,
/// the fallback language will be used. By default the fallback is `en`, but
/// this can be configured.
///
/// # Configuration
///
/// To configure the sources for the languages or the fallback language, see [`UserLanguage::config`].
///
/// # Custom Sources
///
/// You can create custom user language sources. See
/// [`UserLanguageSource`] for details.
///
/// # Example
///
/// ```rust
/// use axum_extra::extract::UserLanguage;
///
/// async fn handler(lang: UserLanguage) {
///     println!("Preferred languages: {:?}", lang.preferred_languages());
/// }
/// ```
#[derive(Debug, Clone)]
pub struct UserLanguage {
    preferred_languages: Vec<String>,
    fallback_language: String,
}

impl UserLanguage {
    /// The default sources for the preferred languages.
    ///
    /// If you do not add a configuration for the [`UserLanguage`] extractor,
    /// these sources will be used by default. They are in order:
    ///  * The query parameter `lang` (if feature `query` is enabled)
    ///  * The path segment `:lang`
    ///  * The `Accept-Language` header
    pub fn default_sources() -> &'static Vec<Arc<dyn UserLanguageSource>> {
        static DEFAULT_SOURCES: OnceLock<Vec<Arc<dyn UserLanguageSource>>> = OnceLock::new();

        DEFAULT_SOURCES.get_or_init(|| {
            vec![
                #[cfg(feature = "query")]
                Arc::new(QuerySource::new("lang")),
                Arc::new(PathSource::new("lang")),
                Arc::new(AcceptLanguageSource),
            ]
        })
    }

    /// The users most preferred language as read from the request.
    ///
    /// This is the first language in the list of [`UserLanguage::preferred_languages`].
    /// If no language could be read from the request, the fallback language
    /// will be returned.
    pub fn preferred_language(&self) -> &str {
        self.preferred_languages
            .first()
            .unwrap_or(&self.fallback_language)
    }

    /// The users preferred languages in order of preference.
    ///
    /// Preference is first determined by the order of the sources.
    /// Within each source the languages are ordered by the users preference,
    /// if applicable for the source. For example the `Accept-Language` header
    /// source will order the languages by the `q` parameter.
    ///
    /// This list may be empty if no language could be read from the request.
    pub fn preferred_languages(&self) -> &[String] {
        self.preferred_languages.as_slice()
    }

    /// The language that will be used as a fallback if no language could be
    /// read from the request.
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
        let (sources, fallback_language) = match parts.extract::<Extension<Config>>().await {
            Ok(Extension(config)) => (Some(config.sources), Some(config.fallback_language)),
            Err(_) => (None, None),
        };

        let sources = sources.as_ref().unwrap_or(Self::default_sources());
        let fallback_language = fallback_language.unwrap_or_else(|| "en".to_owned());

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{routing::get, Router};
    use http::{header::ACCEPT_LANGUAGE, StatusCode};

    #[derive(Debug)]
    struct TestSource(Vec<String>);

    #[async_trait]
    impl UserLanguageSource for TestSource {
        async fn languages_from_parts(&self, _parts: &mut Parts) -> Vec<String> {
            self.0.clone()
        }
    }

    #[tokio::test]
    async fn reads_from_configured_sources_in_specified_order() {
        let app = Router::new()
            .route("/", get(return_all_langs))
            .layer(Extension(
                UserLanguage::config()
                    .add_source(TestSource(vec!["s1.1".to_owned(), "s1.2".to_owned()]))
                    .add_source(TestSource(vec!["s2.1".to_owned(), "s2.2".to_owned()]))
                    .build(),
            ));

        let client = TestClient::new(app);

        let res = client.get("/").send().await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "s1.1,s1.2,s2.1,s2.2");
    }

    #[tokio::test]
    async fn reads_languages_from_default_sources() {
        let app = Router::new().route("/:lang", get(return_all_langs));

        let client = TestClient::new(app);

        let res = client
            .get("/de?lang=fr")
            .header(ACCEPT_LANGUAGE, "en;q=0.9,es;q=0.8")
            .send()
            .await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "fr,de,en,es");
    }

    #[tokio::test]
    async fn falls_back_to_configured_language() {
        let app = Router::new().route("/", get(return_lang)).layer(Extension(
            UserLanguage::config().fallback_language("fallback").build(),
        ));

        let client = TestClient::new(app);

        let res = client.get("/").send().await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "fallback");
    }

    #[tokio::test]
    async fn falls_back_to_default_language() {
        let app = Router::new().route("/", get(return_lang));

        let client = TestClient::new(app);

        let res = client.get("/").send().await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "en");
    }

    async fn return_lang(lang: UserLanguage) -> String {
        lang.preferred_language().to_owned()
    }

    async fn return_all_langs(lang: UserLanguage) -> String {
        lang.preferred_languages().join(",")
    }
}
