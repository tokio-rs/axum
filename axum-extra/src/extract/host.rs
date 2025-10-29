use super::rejection::{FailedToResolveHost, HostRejection};
use axum_core::{
    extract::{FromRequestParts, OptionalFromRequestParts},
    RequestPartsExt,
};
use http::{
    header::{HeaderMap, FORWARDED},
    request::Parts,
    uri::Authority,
};
use std::convert::Infallible;

const X_FORWARDED_HOST_HEADER_KEY: &str = "X-Forwarded-Host";

/// Extractor that resolves the host of the request.
///
/// Host is resolved through the following, in order:
/// - `Forwarded` header
/// - `X-Forwarded-Host` header
/// - `Host` header
/// - Authority of the request URI
///
/// See <https://www.rfc-editor.org/rfc/rfc9110.html#name-host-and-authority> for the definition of
/// host.
///
/// Note that user agents can set `X-Forwarded-Host` and `Host` headers to arbitrary values so make
/// sure to validate them to avoid security issues.
#[derive(Debug, Clone)]
pub struct Host(pub String);

impl<S> FromRequestParts<S> for Host
where
    S: Send + Sync,
{
    type Rejection = HostRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extract::<Option<Self>>()
            .await
            .ok()
            .flatten()
            .ok_or(HostRejection::FailedToResolveHost(FailedToResolveHost))
    }
}

impl<S> OptionalFromRequestParts<S> for Host
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        if let Some(host) = parse_forwarded(&parts.headers) {
            return Ok(Some(Self(host.to_owned())));
        }

        if let Some(host) = parts
            .headers
            .get(X_FORWARDED_HOST_HEADER_KEY)
            .and_then(|host| host.to_str().ok())
        {
            return Ok(Some(Self(host.to_owned())));
        }

        if let Some(host) = parts
            .headers
            .get(http::header::HOST)
            .and_then(|host| host.to_str().ok())
        {
            return Ok(Some(Self(host.to_owned())));
        }

        if let Some(authority) = parts.uri.authority() {
            return Ok(Some(Self(parse_authority(authority).to_owned())));
        }

        Ok(None)
    }
}

#[allow(warnings)]
fn parse_forwarded(headers: &HeaderMap) -> Option<&str> {
    // if there are multiple `Forwarded` `HeaderMap::get` will return the first one
    let forwarded_values = headers.get(FORWARDED)?.to_str().ok()?;

    // get the first set of values
    let first_value = forwarded_values.split(',').nth(0)?;

    // find the value of the `host` field
    first_value.split(';').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("host")
            .then(|| value.trim().trim_matches('"'))
    })
}

fn parse_authority(auth: &Authority) -> &str {
    auth.as_str()
        .rsplit('@')
        .next()
        .expect("split always has at least 1 item")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::TestClient;
    use axum::{routing::get, Router};
    use http::{header::HeaderName, Request};

    fn test_client() -> TestClient {
        async fn host_as_body(Host(host): Host) -> String {
            host
        }

        TestClient::new(Router::new().route("/", get(host_as_body)))
    }

    #[crate::test]
    async fn host_header() {
        let original_host = "some-domain:123";
        let host = test_client()
            .get("/")
            .header(http::header::HOST, original_host)
            .await
            .text()
            .await;
        assert_eq!(host, original_host);
    }

    #[crate::test]
    async fn x_forwarded_host_header() {
        let original_host = "some-domain:456";
        let host = test_client()
            .get("/")
            .header(X_FORWARDED_HOST_HEADER_KEY, original_host)
            .await
            .text()
            .await;
        assert_eq!(host, original_host);
    }

    #[crate::test]
    async fn x_forwarded_host_precedence_over_host_header() {
        let x_forwarded_host_header = "some-domain:456";
        let host_header = "some-domain:123";
        let host = test_client()
            .get("/")
            .header(X_FORWARDED_HOST_HEADER_KEY, x_forwarded_host_header)
            .header(http::header::HOST, host_header)
            .await
            .text()
            .await;
        assert_eq!(host, x_forwarded_host_header);
    }

    #[crate::test]
    async fn uri_host() {
        let client = test_client();
        let port = client.server_port();
        let host = client.get("/").await.text().await;
        assert_eq!(host, format!("127.0.0.1:{port}"));
    }

    #[crate::test]
    async fn ip4_uri_host() {
        let mut parts = Request::new(()).into_parts().0;
        parts.uri = "https://127.0.0.1:1234/image.jpg".parse().unwrap();
        let host = parts.extract::<Host>().await.unwrap();
        assert_eq!(host.0, "127.0.0.1:1234");
    }

    #[crate::test]
    async fn ip6_uri_host() {
        let mut parts = Request::new(()).into_parts().0;
        parts.uri = "http://cool:user@[::1]:456/file.txt".parse().unwrap();
        let host = parts.extract::<Host>().await.unwrap();
        assert_eq!(host.0, "[::1]:456");
    }

    #[crate::test]
    async fn missing_host() {
        let mut parts = Request::new(()).into_parts().0;
        let host = parts.extract::<Host>().await.unwrap_err();
        assert!(matches!(host, HostRejection::FailedToResolveHost(_)));
    }

    #[crate::test]
    async fn optional_extractor() {
        let mut parts = Request::new(()).into_parts().0;
        parts.uri = "https://127.0.0.1:1234/image.jpg".parse().unwrap();
        let host = parts.extract::<Option<Host>>().await.unwrap();
        assert!(host.is_some());
    }

    #[crate::test]
    async fn optional_extractor_none() {
        let mut parts = Request::new(()).into_parts().0;
        let host = parts.extract::<Option<Host>>().await.unwrap();
        assert!(host.is_none());
    }

    #[test]
    fn forwarded_parsing() {
        // the basic case
        let headers = header_map(&[(FORWARDED, "host=192.0.2.60;proto=http;by=203.0.113.43")]);
        let value = parse_forwarded(&headers).unwrap();
        assert_eq!(value, "192.0.2.60");

        // is case insensitive
        let headers = header_map(&[(FORWARDED, "host=192.0.2.60;proto=http;by=203.0.113.43")]);
        let value = parse_forwarded(&headers).unwrap();
        assert_eq!(value, "192.0.2.60");

        // ipv6
        let headers = header_map(&[(FORWARDED, "host=\"[2001:db8:cafe::17]:4711\"")]);
        let value = parse_forwarded(&headers).unwrap();
        assert_eq!(value, "[2001:db8:cafe::17]:4711");

        // multiple values in one header
        let headers = header_map(&[(FORWARDED, "host=192.0.2.60, host=127.0.0.1")]);
        let value = parse_forwarded(&headers).unwrap();
        assert_eq!(value, "192.0.2.60");

        // multiple header values
        let headers = header_map(&[
            (FORWARDED, "host=192.0.2.60"),
            (FORWARDED, "host=127.0.0.1"),
        ]);
        let value = parse_forwarded(&headers).unwrap();
        assert_eq!(value, "192.0.2.60");
    }

    fn header_map(values: &[(HeaderName, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (key, value) in values {
            headers.append(key, value.parse().unwrap());
        }
        headers
    }
}
