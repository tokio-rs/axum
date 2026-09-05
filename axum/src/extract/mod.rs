#![doc = include_str!("../docs/extract.md")]

use http::header::{self, HeaderMap};

/// A result whose error type is the rejection of a [`FromRequestParts`] extractor.
///
/// Use this to handle an extractor's rejection in the handler instead of automatically
/// returning it as a response.
///
/// ```
/// use axum::{extract::{Path, Result}, routing::get, Router};
///
/// async fn handler(path: Result<Path<u64>>) -> String {
///     match path {
///         Ok(Path(id)) => format!("Item {id}"),
///         Err(rejection) => format!("Invalid path: {rejection}"),
///     }
/// }
///
/// let app = Router::new().route("/{id}", get(handler));
/// # let _: Router = app;
/// ```
///
/// `S` is the state type used by the extractor and defaults to `()`. For extractors
/// that require application state, specify it explicitly:
///
/// ```
/// use axum::{extract::{Result, State}, routing::post, Router};
///
/// #[derive(Clone)]
/// struct AppState;
///
/// async fn handler(state: Result<State<AppState>, AppState>, body: String) -> String {
///     let State(state) = state.unwrap();
///     body
/// }
///
/// let app = Router::new().route("/", post(handler)).with_state(AppState);
/// # let _: Router = app;
/// ```
///
/// This alias is for request-parts extractors. For body-consuming extractors such
/// as `Json`, use [`std::result::Result`] with their rejection type.
pub type Result<T, S = ()> = std::result::Result<T, <T as FromRequestParts<S>>::Rejection>;

#[cfg(feature = "tokio")]
pub mod connect_info;
pub mod path;
pub mod rejection;

#[cfg(feature = "ws")]
pub mod ws;

pub(crate) mod nested_path;
#[cfg(feature = "original-uri")]
mod original_uri;
mod raw_form;
mod raw_query;
mod state;

#[doc(inline)]
pub use axum_core::extract::{
    DefaultBodyLimit, FromRef, FromRequest, FromRequestParts, OptionalFromRequest,
    OptionalFromRequestParts, Request,
};

#[cfg(feature = "macros")]
pub use axum_macros::{FromRef, FromRequest, FromRequestParts};

#[doc(inline)]
pub use self::{
    nested_path::NestedPath,
    path::{Path, RawPathParams},
    raw_form::RawForm,
    raw_query::RawQuery,
    state::State,
};

#[doc(inline)]
#[cfg(feature = "tokio")]
pub use self::connect_info::ConnectInfo;

#[doc(no_inline)]
#[cfg(feature = "json")]
pub use crate::Json;

#[doc(no_inline)]
pub use crate::Extension;

#[cfg(feature = "form")]
#[doc(no_inline)]
pub use crate::form::Form;

#[cfg(feature = "matched-path")]
pub(crate) mod matched_path;

#[cfg(feature = "matched-path")]
#[doc(inline)]
pub use self::matched_path::MatchedPath;

#[cfg(feature = "multipart")]
pub mod multipart;

#[cfg(feature = "multipart")]
#[doc(inline)]
pub use self::multipart::Multipart;

#[cfg(feature = "query")]
mod query;

#[cfg(feature = "query")]
#[doc(inline)]
pub use self::query::Query;

#[cfg(feature = "original-uri")]
#[doc(inline)]
pub use self::original_uri::OriginalUri;

#[cfg(feature = "ws")]
#[doc(inline)]
pub use self::ws::WebSocketUpgrade;

// this is duplicated in `axum-extra/src/extract/form.rs`
pub(super) fn has_content_type(headers: &HeaderMap, expected_content_type: &mime::Mime) -> bool {
    let Some(content_type) = headers.get(header::CONTENT_TYPE) else {
        return false;
    };

    let Ok(content_type) = content_type.to_str() else {
        return false;
    };

    content_type.starts_with(expected_content_type.as_ref())
}

#[cfg(test)]
mod tests {
    use super::{Path, Result, State};
    use crate::{routing::get, test_helpers::*, Router};
    use http::StatusCode;

    #[crate::test]
    async fn result_alias_handles_path_rejection() {
        let app = Router::new().route(
            "/{id}",
            get(|path: Result<Path<u64>>| async move {
                match path {
                    Ok(Path(id)) => format!("Item {id}"),
                    Err(_) => "Invalid path".to_owned(),
                }
            }),
        );
        let client = TestClient::new(app);
        assert_eq!(client.get("/42").await.text().await, "Item 42");
        let response = client.get("/invalid").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await, "Invalid path");
    }

    #[crate::test]
    async fn result_alias_with_state_and_body() {
        #[derive(Clone)]
        struct AppState(&'static str);

        let app = Router::new()
            .route(
                "/",
                get(
                    |state: Result<State<AppState>, AppState>, body: String| async move {
                        let State(state) = state.unwrap();
                        format!("{}: {body}", state.0)
                    },
                ),
            )
            .with_state(AppState("state"));
        let client = TestClient::new(app);
        assert_eq!(
            client.get("/").body("body").await.text().await,
            "state: body"
        );
    }

    #[crate::test]
    async fn consume_body() {
        let app = Router::new().route("/", get(|body: String| async { body }));

        let client = TestClient::new(app);
        let res = client.get("/").body("foo").await;
        let body = res.text().await;

        assert_eq!(body, "foo");
    }
}
