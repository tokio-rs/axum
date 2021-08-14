use super::{has_content_type, rejection::*, take_body, FromRequest, RequestParts};
use async_trait::async_trait;
use bytes::Buf;
use http::Method;
use serde::de::DeserializeOwned;
use std::ops::Deref;
use tower::BoxError;

/// Extractor that deserializes `application/x-www-form-urlencoded` requests
/// into some type.
///
/// `T` is expected to implement [`serde::Deserialize`].
///
/// # Example
///
/// ```rust,no_run
/// use axum::prelude::*;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct SignUp {
///     username: String,
///     password: String,
/// }
///
/// async fn accept_form(form: extract::Form<SignUp>) {
///     let sign_up: SignUp = form.0;
///
///     // ...
/// }
///
/// let app = route("/sign_up", post(accept_form));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Note that `Content-Type: multipart/form-data` requests are not supported.
#[derive(Debug, Clone, Copy, Default)]
pub struct Form<T>(pub T);

#[async_trait]
impl<T, B> FromRequest<B> for Form<T>
where
    T: DeserializeOwned,
    B: http_body::Body + Send,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    type Rejection = FormRejection;

    #[allow(warnings)]
    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        if req.method() == Method::GET {
            let query = req.uri().query().unwrap_or_default();
            let value = serde_urlencoded::from_str(query)
                .map_err(FailedToDeserializeQueryString::new::<T, _>)?;
            Ok(Form(value))
        } else {
            if !has_content_type(&req, "application/x-www-form-urlencoded")? {
                Err(InvalidFormContentType)?;
            }

            let body = take_body(req)?;
            let chunks = hyper::body::aggregate(body)
                .await
                .map_err(FailedToBufferBody::from_err)?;
            let value = serde_urlencoded::from_reader(chunks.reader())
                .map_err(FailedToDeserializeQueryString::new::<T, _>)?;

            Ok(Form(value))
        }
    }
}

impl<T> Deref for Form<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::RequestParts;
    use http::Request;
    use serde::{Deserialize, Serialize};
    use std::fmt::Debug;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Pagination {
        size: Option<u64>,
        page: Option<u64>,
    }

    async fn check_query<T: DeserializeOwned + PartialEq + Debug>(uri: impl AsRef<str>, value: T) {
        let mut req = RequestParts::new(
            Request::builder()
                .uri(uri.as_ref())
                .body(http_body::Empty::<bytes::Bytes>::new())
                .unwrap(),
        );
        assert_eq!(Form::<T>::from_request(&mut req).await.unwrap().0, value);
    }

    async fn check_body<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: T) {
        let mut req = RequestParts::new(
            Request::builder()
                .uri("http://example.com/test")
                .method(Method::POST)
                .header(
                    http::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(http_body::Full::<bytes::Bytes>::new(
                    serde_urlencoded::to_string(&value).unwrap().into(),
                ))
                .unwrap(),
        );
        assert_eq!(Form::<T>::from_request(&mut req).await.unwrap().0, value);
    }

    #[tokio::test]
    async fn test_form_query() {
        check_query(
            "http://example.com/test",
            Pagination {
                size: None,
                page: None,
            },
        )
        .await;

        check_query(
            "http://example.com/test?size=10",
            Pagination {
                size: Some(10),
                page: None,
            },
        )
        .await;

        check_query(
            "http://example.com/test?size=10&page=20",
            Pagination {
                size: Some(10),
                page: Some(20),
            },
        )
        .await;
    }

    #[tokio::test]
    async fn test_form_body() {
        check_body(Pagination {
            size: None,
            page: None,
        })
        .await;

        check_body(Pagination {
            size: Some(10),
            page: None,
        })
        .await;

        check_body(Pagination {
            size: Some(10),
            page: Some(20),
        })
        .await;
    }

    #[tokio::test]
    async fn test_incorrect_content_type() {
        let mut req = RequestParts::new(
            Request::builder()
                .uri("http://example.com/test")
                .method(Method::POST)
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(http_body::Full::<bytes::Bytes>::new(
                    serde_urlencoded::to_string(&Pagination {
                        size: Some(10),
                        page: None,
                    })
                    .unwrap()
                    .into(),
                ))
                .unwrap(),
        );
        assert!(matches!(
            Form::<Pagination>::from_request(&mut req)
                .await
                .unwrap_err(),
            FormRejection::InvalidFormContentType(InvalidFormContentType)
        ));
    }
}
