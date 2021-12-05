#![doc = include_str!("../docs/extract.md")]

use http::header;
use rejection::*;

pub mod connect_info;
pub mod extractor_middleware;
pub mod path;
pub mod rejection;

#[cfg(feature = "ws")]
#[cfg_attr(docsrs, doc(cfg(feature = "ws")))]
pub mod ws;

mod content_length_limit;
mod extension;
mod form;
mod matched_path;
mod query;
mod raw_query;
mod request_parts;

#[doc(inline)]
pub use axum_core::extract::{FromRequest, RequestParts};

#[doc(inline)]
#[allow(deprecated)]
pub use self::{
    connect_info::ConnectInfo,
    content_length_limit::ContentLengthLimit,
    extension::Extension,
    extractor_middleware::extractor_middleware,
    form::Form,
    matched_path::MatchedPath,
    path::Path,
    query::Query,
    raw_query::RawQuery,
    request_parts::OriginalUri,
    request_parts::{BodyStream, RawBody},
};
#[doc(no_inline)]
#[cfg(feature = "json")]
pub use crate::Json;

#[cfg(feature = "multipart")]
#[cfg_attr(docsrs, doc(cfg(feature = "multipart")))]
pub mod multipart;

#[cfg(feature = "multipart")]
#[cfg_attr(docsrs, doc(cfg(feature = "multipart")))]
#[doc(inline)]
pub use self::multipart::Multipart;

#[cfg(feature = "ws")]
#[cfg_attr(docsrs, doc(cfg(feature = "ws")))]
#[doc(inline)]
pub use self::ws::WebSocketUpgrade;

#[cfg(feature = "headers")]
#[cfg_attr(docsrs, doc(cfg(feature = "headers")))]
mod typed_header;

#[cfg(feature = "headers")]
#[cfg_attr(docsrs, doc(cfg(feature = "headers")))]
#[doc(inline)]
pub use self::typed_header::TypedHeader;

pub(crate) fn has_content_type<B>(
    req: &RequestParts<B>,
    expected_content_type: &mime::Mime,
) -> Result<bool, HeadersAlreadyExtracted> {
    let content_type = if let Some(content_type) = req
        .headers()
        .ok_or_else(HeadersAlreadyExtracted::default)?
        .get(header::CONTENT_TYPE)
    {
        content_type
    } else {
        return Ok(false);
    };

    let content_type = if let Ok(content_type) = content_type.to_str() {
        content_type
    } else {
        return Ok(false);
    };

    Ok(content_type.starts_with(expected_content_type.as_ref()))
}

pub(crate) fn take_body<B>(req: &mut RequestParts<B>) -> Result<B, BodyAlreadyExtracted> {
    req.take_body().ok_or_else(BodyAlreadyExtracted::default)
}

#[cfg(test)]
mod tests {
    use crate::{routing::get, test_helpers::*, Router};

    #[tokio::test]
    async fn consume_body() {
        let app = Router::new().route("/", get(|body: String| async { body }));

        let client = TestClient::new(app);
        let res = client.get("/").body("foo").send().await;
        let body = res.text().await;

        assert_eq!(body, "foo");
    }
}
