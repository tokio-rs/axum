use super::{
    rejection::{FailedToResolveHost, HostRejection},
    FromRequest, RequestParts,
};
use async_trait::async_trait;

const X_FORWARDED_HOST_HEADER_KEY: &str = "X-Forwarded-Host";

/// Extractor that resolves the hostname of the request.
///
/// Hostname is resolved through the following, in order:
/// - `X-Forwarded-Host` header
/// - `Host` header
/// - request target / URI
///
/// Note that user agents can set `X-Forwarded-Host` and `Host` headers to arbitrary values so make
/// sure to validate them to avoid security issues.
#[derive(Debug, Clone)]
pub struct Host(pub String);

#[async_trait]
impl<B> FromRequest<B> for Host
where
    B: Send,
{
    type Rejection = HostRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        // todo: extract host from http::header::FORWARDED

        if let Some(host) = req
            .headers()
            .get(X_FORWARDED_HOST_HEADER_KEY)
            .and_then(|host| host.to_str().ok())
        {
            return Ok(Host(host.to_owned()));
        }

        if let Some(host) = req
            .headers()
            .get(http::header::HOST)
            .and_then(|host| host.to_str().ok())
        {
            return Ok(Host(host.to_owned()));
        }

        if let Some(host) = req.uri().host() {
            return Ok(Host(host.to_owned()));
        }

        Err(HostRejection::FailedToResolveHost(FailedToResolveHost))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{routing::get, test_helpers::TestClient, Router};

    fn test_client() -> TestClient {
        async fn host_as_body(Host(host): Host) -> String {
            host
        }

        TestClient::new(Router::new().route("/", get(host_as_body)))
    }

    #[tokio::test]
    async fn host_header() {
        let original_host = "some-domain:123";
        let host = test_client()
            .get("/")
            .header(http::header::HOST, original_host)
            .send()
            .await
            .text()
            .await;
        assert_eq!(host, original_host);
    }

    #[tokio::test]
    async fn x_forwarded_host_header() {
        let original_host = "some-domain:456";
        let host = test_client()
            .get("/")
            .header(X_FORWARDED_HOST_HEADER_KEY, original_host)
            .send()
            .await
            .text()
            .await;
        assert_eq!(host, original_host);
    }

    #[tokio::test]
    async fn x_forwarded_host_precedence_over_host_header() {
        let x_forwarded_host_header = "some-domain:456";
        let host_header = "some-domain:123";
        let host = test_client()
            .get("/")
            .header(X_FORWARDED_HOST_HEADER_KEY, x_forwarded_host_header)
            .header(http::header::HOST, host_header)
            .send()
            .await
            .text()
            .await;
        assert_eq!(host, x_forwarded_host_header);
    }

    #[tokio::test]
    async fn uri_host() {
        let host = test_client().get("/").send().await.text().await;
        assert!(host.contains("127.0.0.1"));
    }
}
