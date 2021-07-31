//! Various helpers for Axum applications to use during testing.

use http::Response;
use serde::de::DeserializeOwned;
pub use tower::ServiceExt;

use crate::body::BoxBody;

/// Helper function that returns a deserialized response body of a TestRequest
///
/// ```
/// use axum::{prelude::*, routing::BoxRoute};
/// use axum::test::*;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize)]
/// pub struct Person {
///     name: String
/// }
///
/// fn app() -> BoxRoute<Body> {
///    route(
///         "/json",
///         post(|| async move {
///                response::Json(Person{ name: "MyName".to_owned()})
///         }),
///        )
///        .boxed()
/// }
/// #[tokio::test]
/// async fn json() {
///     let app = app();
/// 
///     let response = app
///         .oneshot(
///             Request::builder()
///                 .method(http::Method::POST)
///                 .uri("/json")
///                 .header(http::header::CONTENT_TYPE, "application/json")
///                 .unwrap(),
///         )
///         .await
///         .unwrap();
/// 
///     assert_eq!(response.status(), StatusCode::OK);
///
///     let result: Person = test::read_response_json(response).await;
///     assert_eq!("MyName", &result.name)
/// }
/// ```
pub async fn read_response_json<T>(response: Response<BoxBody>) -> T
where
    T: DeserializeOwned,
{
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap_or_else(|err| {
        panic!(
            "read_response_json failed during extraction of body. Err: {:?}",
            err
        )
    });
    serde_json::from_slice(&body).unwrap_or_else(|err| {
        panic!(
            "read_response_json failed during deserialization of body: {:?}. Err: {:?}",
            body, err
        )
    })
}