use async_trait::async_trait;
use axum_core::extract::FromRequest;
use http::{Method, Request};

use super::{
    has_content_type,
    rejection::{InvalidFormContentType, RawFormRejection},
};

use crate::{body::HttpBody, BoxError};

/// Extractor that extracts the raw form string, without parsing it.
///
/// # Example
/// ```rust,no_run
/// use axum::{
///     extract::RawForm,
///     routing::get,
///     Router
/// };
///
/// async fn handler(RawForm(form): RawForm) {}
///
/// let router = Router::new().route("/", get(handler));
/// ```
#[derive(Debug)]
pub struct RawForm(pub Option<String>);

#[async_trait]
impl<S, B> FromRequest<S, B> for RawForm
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
    S: Send + Sync,
{
    type Rejection = RawFormRejection;

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        if req.method() == Method::GET {
            Ok(Self(req.uri().query().map(String::from)))
        } else {
            if !has_content_type(req.headers(), &mime::APPLICATION_WWW_FORM_URLENCODED) {
                return Err(InvalidFormContentType.into());
            }

            Ok(Self(Some(String::from_request(req, state).await?)))
        }
    }
}
