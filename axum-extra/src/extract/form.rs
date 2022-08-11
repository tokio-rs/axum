use axum::{
    async_trait,
    body::HttpBody,
    extract::{
        rejection::{FailedToDeserializeQueryString, FormRejection, InvalidFormContentType},
        FromRequest, RequestParts,
    },
    BoxError,
};
use bytes::Bytes;
use http::{header, Method};
use serde::de::DeserializeOwned;
use std::ops::Deref;

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

impl<T> Deref for Form<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait]
impl<T, S, B> FromRequest<B, S> for Form<T>
where
    T: DeserializeOwned,
    B: HttpBody + Send,
    B::Data: Send,
    B::Error: Into<BoxError>,
    S: Send,
{
    type Rejection = FormRejection;

    async fn from_request(req: &mut RequestParts<B, S>) -> Result<Self, Self::Rejection> {
        if req.method() == Method::GET {
            let query = req.uri().query().unwrap_or_default();
            let value = serde_html_form::from_str(query)
                .map_err(FailedToDeserializeQueryString::__private_new)?;
            Ok(Form(value))
        } else {
            if !has_content_type(req, &mime::APPLICATION_WWW_FORM_URLENCODED) {
                return Err(InvalidFormContentType::default().into());
            }

            let bytes = Bytes::from_request(req).await?;
            let value = serde_html_form::from_bytes(&bytes)
                .map_err(FailedToDeserializeQueryString::__private_new)?;

            Ok(Form(value))
        }
    }
}

// this is duplicated in `axum/src/extract/mod.rs`
fn has_content_type<B, S>(req: &RequestParts<B, S>, expected_content_type: &mime::Mime) -> bool {
    let content_type = if let Some(content_type) = req.headers().get(header::CONTENT_TYPE) {
        content_type
    } else {
        return false;
    };

    let content_type = if let Ok(content_type) = content_type.to_str() {
        content_type
    } else {
        return false;
    };

    content_type.starts_with(expected_content_type.as_ref())
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
