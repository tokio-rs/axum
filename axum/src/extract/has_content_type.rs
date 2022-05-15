// this is in its own file such that we can share it between `axum` and `axum-extra` without making
// it part of the public API. We get into `axum-extra` using `include!`.

use super::RequestParts;
use http::header;

pub(crate) fn has_content_type<B>(
    req: &RequestParts<B>,
    expected_content_type: &mime::Mime,
) -> bool {
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
