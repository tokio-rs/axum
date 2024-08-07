use axum::async_trait;
use http::request::Parts;
use std::fmt::Debug;

/// A source for the users preferred languages.
///
/// # Implementing a custom source
///
/// The following is an example of how to read the language from the query.
///
/// ```rust
/// use std::collections::HashMap;
/// use axum::{extract::Query, RequestPartsExt};
/// use axum_extra::extract::user_lang::UserLanguageSource;
///
/// #[derive(Debug)]
/// pub struct QuerySource;
///
/// #[axum::async_trait]
/// impl UserLanguageSource for QuerySource {
///     async fn languages_from_parts(&self, parts: &mut http::request::Parts) -> Vec<String> {
///         let Ok(query) = parts.extract::<Query<HashMap<String, String>>>().await else {
///             return vec![];
///         };
///
///         let Some(lang) = query.get("lang") else {
///             return vec![];
///         };
///
///         vec![lang.to_owned()]
///     }
/// }
/// ```
#[async_trait]
pub trait UserLanguageSource: Send + Sync + Debug {
    /// Extract a list of user languages from the request parts.
    ///
    /// The multiple languages are returned, they should be in
    /// order of preference of the user, if possible.
    ///
    /// If no languages could be read from the request, return
    /// an empty vec.
    async fn languages_from_parts(&self, parts: &mut Parts) -> Vec<String>;
}
