use axum::{
    async_trait,
    body::HttpBody,
    extract::{rejection::RawFormRejection, FromRequest, RawForm},
    response::{IntoResponse, Response},
    BoxError, Error, RequestExt,
};
use http::{Request, StatusCode};
use serde::de::DeserializeOwned;
use std::fmt;

/// Extractor that deserializes `application/x-www-form-urlencoded` requests
/// into some type.
///
/// `T` is expected to implement [`serde::Deserialize`].
///
/// # Differences from `axum::extract::Form`
///
/// This extractor uses [`serde_html_form`] under-the-hood which supports multi-value items. These
/// are sent by multiple `<input>` attributes of the same name (e.g. checkboxes) and `<select>`s
/// with the `multiple` attribute. Those values can be collected into a `Vec` or other sequential
/// container.
///
/// # Example
///
/// ```rust,no_run
/// use axum_extra::extract::Form;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Payload {
///     #[serde(rename = "value")]
///     values: Vec<String>,
/// }
///
/// async fn accept_form(Form(payload): Form<Payload>) {
///     // ...
/// }
/// ```
///
/// [`serde_html_form`]: https://crates.io/crates/serde_html_form
#[derive(Debug, Clone, Copy, Default)]
#[cfg(feature = "form")]
pub struct Form<T>(pub T);

axum_core::__impl_deref!(Form);

#[async_trait]
impl<T, S, B> FromRequest<S, B> for Form<T>
where
    T: DeserializeOwned,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
    S: Send + Sync,
{
    type Rejection = FormRejection;

    async fn from_request(req: Request<B>, _state: &S) -> Result<Self, Self::Rejection> {
        let RawForm(bytes) = req
            .extract()
            .await
            .map_err(FormRejection::RawFormRejection)?;

        serde_html_form::from_bytes::<T>(&bytes)
            .map(Self)
            .map_err(|err| FormRejection::FailedToDeserializeForm(Error::new(err)))
    }
}

/// Rejection used for [`Form`].
///
/// Contains one variant for each way the [`Form`] extractor can fail.
#[derive(Debug)]
#[non_exhaustive]
#[cfg(feature = "form")]
pub enum FormRejection {
    #[allow(missing_docs)]
    RawFormRejection(RawFormRejection),
    #[allow(missing_docs)]
    FailedToDeserializeForm(Error),
}

impl IntoResponse for FormRejection {
    fn into_response(self) -> Response {
        match self {
            Self::RawFormRejection(inner) => inner.into_response(),
            Self::FailedToDeserializeForm(inner) => (
                StatusCode::BAD_REQUEST,
                format!("Failed to deserialize form: {}", inner),
            )
                .into_response(),
        }
    }
}

impl fmt::Display for FormRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RawFormRejection(inner) => inner.fmt(f),
            Self::FailedToDeserializeForm(inner) => inner.fmt(f),
        }
    }
}

impl std::error::Error for FormRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RawFormRejection(inner) => Some(inner),
            Self::FailedToDeserializeForm(inner) => Some(inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{routing::post, Router};
    use http::{header::CONTENT_TYPE, StatusCode};
    use serde::Deserialize;

    #[tokio::test]
    async fn supports_multiple_values() {
        #[derive(Deserialize)]
        struct Data {
            #[serde(rename = "value")]
            values: Vec<String>,
        }

        let app = Router::new().route(
            "/",
            post(|Form(data): Form<Data>| async move { data.values.join(",") }),
        );

        let client = TestClient::new(app);

        let res = client
            .post("/")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body("value=one&value=two")
            .send()
            .await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "one,two");
    }
}
