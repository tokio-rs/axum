use super::{FromRequest, RequestParts, rejection::{HostRejection, FailedToResolveHost}};
use async_trait::async_trait;

/// Extractor that extracts the host from a request.
#[derive(Debug, Clone)]
pub struct Host(pub String);

#[async_trait]
impl<B> FromRequest<B> for Host
where
    B: Send,
{
    type Rejection = HostRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        if let Some(host) = req.headers().get(http::header::FORWARDED).and_then(|host| host.to_str().ok()) {
            return Ok(Host(host.to_owned()));
        }

        if let Some(host) = req.headers().get("X-Forwarded-Host").and_then(|host| host.to_str().ok()) {
            return Ok(Host(host.to_owned()));
        }

        if let Some(host) = req.headers().get(http::header::HOST).and_then(|host| host.to_str().ok()) {
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
    use crate::extract::RequestParts;
    use http::Request;

    #[tokio::test]
    async fn test_host() {
        let mut req = RequestParts::new(
            Request::builder()
                .uri("http://example.com/test")
                .body(())
                .unwrap(),
        );
        assert_eq!(
            &Host::from_request(&mut req).await.unwrap().0,
            "example.com"
        );

        let mut req = RequestParts::new(
            Request::builder()
                .header("host", "cats.fun")
                .body(())
                .unwrap(),
        );
        assert_eq!(
            &Host::from_request(&mut req).await.unwrap().0,
            "cats.fun"
        );
    }
}
