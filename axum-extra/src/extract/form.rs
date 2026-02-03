#![allow(deprecated)]

use axum::extract::rejection::RawFormRejection;
use axum::{
    extract::{FromRequest, RawForm, Request},
    RequestExt,
};
use axum_core::__composite_rejection as composite_rejection;
use axum_core::__define_rejection as define_rejection;
use serde_core::de::DeserializeOwned;

/// Extractor that deserializes `application/x-www-form-urlencoded` requests
/// into some type.
///
/// `T` is expected to implement [`serde::Deserialize`].
///
/// # Deprecated
///
/// This extractor used to use a different deserializer under-the-hood but that
/// is no longer the case. Now it only uses an older version of the same
/// deserializer, purely for ease of transition to the latest version.
/// Before switching to `axum::extract::Form`, it is recommended to read the
/// [changelog for `serde_html_form v0.3.0`][changelog].
///
/// [changelog]: https://github.com/jplatte/serde_html_form/blob/main/CHANGELOG.md#030
#[deprecated = "see documentation"]
#[derive(Debug, Clone, Copy, Default)]
#[cfg(feature = "form")]
pub struct Form<T>(pub T);

axum_core::__impl_deref!(Form);

impl<T, S> FromRequest<S> for Form<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = FormRejection;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let is_get_or_head =
            req.method() == http::Method::GET || req.method() == http::Method::HEAD;

        let RawForm(bytes) = req.extract().await?;

        let deserializer = serde_html_form::Deserializer::new(form_urlencoded::parse(&bytes));

        serde_path_to_error::deserialize::<_, T>(deserializer)
            .map(Self)
            .map_err(|err| {
                if is_get_or_head {
                    FailedToDeserializeForm::from_err(err).into()
                } else {
                    FailedToDeserializeFormBody::from_err(err).into()
                }
            })
    }
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to deserialize form"]
    /// Rejection type used if the [`Form`](Form) extractor is unable to
    /// deserialize the form into the target type.
    pub struct FailedToDeserializeForm(Error);
}

define_rejection! {
    #[status = UNPROCESSABLE_ENTITY]
    #[body = "Failed to deserialize form body"]
    /// Rejection type used if the [`Form`](Form) extractor is unable to
    /// deserialize the form body into the target type.
    pub struct FailedToDeserializeFormBody(Error);
}

composite_rejection! {
    /// Rejection used for [`Form`].
    ///
    /// Contains one variant for each way the [`Form`] extractor can fail.
    #[deprecated = "because Form is deprecated"]
    pub enum FormRejection {
        RawFormRejection,
        FailedToDeserializeForm,
        FailedToDeserializeFormBody,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::routing::{on, post, MethodFilter};
    use axum::Router;
    use http::header::CONTENT_TYPE;
    use http::StatusCode;
    use mime::APPLICATION_WWW_FORM_URLENCODED;
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
            .await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "one,two");
    }

    #[tokio::test]
    async fn deserialize_error_status_codes() {
        #[allow(dead_code)]
        #[derive(Deserialize)]
        struct Payload {
            a: i32,
        }

        let app = Router::new().route(
            "/",
            on(
                MethodFilter::GET.or(MethodFilter::POST),
                |_: Form<Payload>| async {},
            ),
        );

        let client = TestClient::new(app);

        let res = client.get("/?a=false").await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            res.text().await,
            "Failed to deserialize form: a: invalid digit found in string"
        );

        let res = client
            .post("/")
            .header(CONTENT_TYPE, APPLICATION_WWW_FORM_URLENCODED.as_ref())
            .body("a=false")
            .await;
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            res.text().await,
            "Failed to deserialize form body: a: invalid digit found in string"
        );
    }
}
