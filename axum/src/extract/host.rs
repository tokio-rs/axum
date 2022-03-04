use super::{FromRequest, RequestParts};
use async_trait::async_trait;
use std::{convert::Infallible};

/// Extractor that extracts the host from a request.
#[derive(Debug, Clone, Default)]
pub struct Host(pub String);

#[async_trait]
impl<B> FromRequest<B> for Host
where
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        if let Some(host) = req.uri().host() {
            return Ok(Host(host.to_string()));
        }

        if let Some(Ok(host)) = req.headers().get("host").map(|host| host.to_str()) {
            return Ok(Host(host.to_string()));
        }

        Ok(Host("".to_string()))
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
