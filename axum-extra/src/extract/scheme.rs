//! Extractor that parses the scheme of a request.
//! See [`Scheme`] for more details.

use axum::extract::FromRequestParts;
use axum_core::__define_rejection as define_rejection;
use http::{
    header::{HeaderMap, FORWARDED},
    request::Parts,
};
const X_FORWARDED_PROTO_HEADER_KEY: &str = "X-Forwarded-Proto";

/// Extractor that resolves the scheme / protocol of a request.
///
/// The scheme is resolved through the following, in order:
/// - `Forwarded` header
/// - `X-Forwarded-Proto` header
/// - Request URI (If the request is an HTTP/2 request! e.g. use `--http2(-prior-knowledge)` with cURL)
///
/// Note that user agents can set the `X-Forwarded-Proto` header to arbitrary values so make
/// sure to validate them to avoid security issues.
#[derive(Debug, Clone)]
pub struct Scheme(pub String);

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "No scheme found in request"]
    /// Rejection type used if the [`Scheme`] extractor is unable to
    /// resolve a scheme.
    pub struct SchemeMissing;
}

impl<S> FromRequestParts<S> for Scheme
where
    S: Send + Sync,
{
    type Rejection = SchemeMissing;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Within Forwarded header
        if let Some(scheme) = parse_forwarded(&parts.headers) {
            return Ok(Scheme(scheme.to_owned()));
        }

        // X-Forwarded-Proto
        if let Some(scheme) = parts
            .headers
            .get(X_FORWARDED_PROTO_HEADER_KEY)
            .and_then(|scheme| scheme.to_str().ok())
        {
            return Ok(Scheme(scheme.to_owned()));
        }

        // From parts of an HTTP/2 request
        if let Some(scheme) = parts.uri.scheme_str() {
            return Ok(Scheme(scheme.to_owned()));
        }

        Err(SchemeMissing)
    }
}

fn parse_forwarded(headers: &HeaderMap) -> Option<&str> {
    // if there are multiple `Forwarded` `HeaderMap::get` will return the first one
    let forwarded_values = headers.get(FORWARDED)?.to_str().ok()?;

    // get the first set of values
    let first_value = forwarded_values.split(',').next()?;

    // find the value of the `proto` field
    first_value.split(';').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("proto")
            .then(|| value.trim().trim_matches('"'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::TestClient;
    use axum::{routing::get, Router};
    use http::header::HeaderName;

    fn test_client() -> TestClient {
        async fn scheme_as_body(Scheme(scheme): Scheme) -> String {
            scheme
        }

        TestClient::new(Router::new().route("/", get(scheme_as_body)))
    }

    #[crate::test]
    async fn forwarded_scheme_parsing() {
        // the basic case
        let headers = header_map(&[(FORWARDED, "host=192.0.2.60;proto=http;by=203.0.113.43")]);
        let value = parse_forwarded(&headers).unwrap();
        assert_eq!(value, "http");

        // is case insensitive
        let headers = header_map(&[(FORWARDED, "host=192.0.2.60;PROTO=https;by=203.0.113.43")]);
        let value = parse_forwarded(&headers).unwrap();
        assert_eq!(value, "https");

        // multiple values in one header
        let headers = header_map(&[(FORWARDED, "proto=ftp, proto=https")]);
        let value = parse_forwarded(&headers).unwrap();
        assert_eq!(value, "ftp");

        // multiple header values
        let headers = header_map(&[(FORWARDED, "proto=ftp"), (FORWARDED, "proto=https")]);
        let value = parse_forwarded(&headers).unwrap();
        assert_eq!(value, "ftp");
    }

    #[crate::test]
    async fn x_forwarded_scheme_header() {
        let original_scheme = "https";
        let scheme = test_client()
            .get("/")
            .header(X_FORWARDED_PROTO_HEADER_KEY, original_scheme)
            .await
            .text()
            .await;
        assert_eq!(scheme, original_scheme);
    }

    #[crate::test]
    async fn precedence_forwarded_over_x_forwarded() {
        let scheme = test_client()
            .get("/")
            .header(X_FORWARDED_PROTO_HEADER_KEY, "https")
            .header(FORWARDED, "proto=ftp")
            .await
            .text()
            .await;
        assert_eq!(scheme, "ftp");
    }

    fn header_map(values: &[(HeaderName, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (key, value) in values {
            headers.append(key, value.parse().unwrap());
        }
        headers
    }
}
