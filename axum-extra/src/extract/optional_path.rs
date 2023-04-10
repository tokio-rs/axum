use axum::{
    async_trait,
    extract::{path::ErrorKind, rejection::PathRejection, FromRequestParts, Path},
    RequestPartsExt,
};
use serde::de::DeserializeOwned;

/// Extractor that extracts path arguments the same way as [`Path`], except if there aren't any.
///
/// This extractor can be used in place of `Path` when you have two routes that you want to handle
/// in mostly the same way, where one has a path parameter and the other one doesn't.
///
/// # Example
///
/// ```
/// use std::num::NonZeroU32;
/// use axum::{
///     response::IntoResponse,
///     routing::get,
///     Router,
/// };
/// use axum_extra::extract::OptionalPath;
///
/// async fn render_blog(OptionalPath(page): OptionalPath<NonZeroU32>) -> impl IntoResponse {
///     // Convert to u32, default to page 1 if not specified
///     let page = page.map_or(1, |param| param.get());
///     // ...
/// }
///
/// let app = Router::new()
///     .route("/blog", get(render_blog))
///     .route("/blog/:page", get(render_blog));
/// # let app: Router = app;
/// ```
#[derive(Debug)]
pub struct OptionalPath<T>(pub Option<T>);

#[async_trait]
impl<T, S> FromRequestParts<S> for OptionalPath<T>
where
    T: DeserializeOwned + Send + 'static,
    S: Send + Sync,
{
    type Rejection = PathRejection;

    async fn from_request_parts(
        parts: &mut http::request::Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        match parts.extract::<Path<T>>().await {
            Ok(Path(params)) => Ok(Self(Some(params))),
            Err(PathRejection::FailedToDeserializePathParams(e))
                if matches!(e.kind(), ErrorKind::WrongNumberOfParameters { got: 0, .. }) =>
            {
                Ok(Self(None))
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use axum::{routing::get, Router};

    use super::OptionalPath;
    use crate::test_helpers::TestClient;

    #[crate::test]
    async fn supports_128_bit_numbers() {
        async fn handle(OptionalPath(param): OptionalPath<NonZeroU32>) -> String {
            let num = param.map_or(0, |p| p.get());
            format!("Success: {num}")
        }

        let app = Router::new()
            .route("/", get(handle))
            .route("/:num", get(handle));

        let client = TestClient::new(app);

        let res = client.get("/").send().await;
        assert_eq!(res.text().await, "Success: 0");

        let res = client.get("/1").send().await;
        assert_eq!(res.text().await, "Success: 1");

        let res = client.get("/0").send().await;
        assert_eq!(
            res.text().await,
            "Invalid URL: invalid value: integer `0`, expected a nonzero u32"
        );

        let res = client.get("/NaN").send().await;
        assert_eq!(
            res.text().await,
            "Invalid URL: Cannot parse `\"NaN\"` to a `u32`"
        );
    }
}
