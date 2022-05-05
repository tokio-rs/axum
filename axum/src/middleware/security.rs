use crate::{body::Body, http::Request, response::Response};
use futures_util::future::BoxFuture;
use headers::{ HeaderMap, HeaderName, HeaderValue};
use std::task::{Context, Poll};
use tower::{Layer, Service};

pub(crate) trait Policy {
    const HEADER_NAME: &'static str;
    fn header(&self) -> (HeaderName, HeaderValue);
}

pub(crate) struct Security(HeaderMap);

impl Security {
    pub(crate) fn new() -> Self {
        Self(HeaderMap::new())
    }
    pub(crate) fn with(mut self, policy: impl Policy) -> Self {
        let (key, value) = policy.header();
        if self.0.contains_key(&key) {
            let _ = self.0.remove(&key);
        }
        self.0.insert(key, value);
        self
    }
}

enum XssFilter {
    Disable,
    Enable,
    ModeBlock,
}

impl Policy for XssFilter {
    const HEADER_NAME: &'static str = "x-xss-protection";

    fn header(&self) -> (HeaderName, HeaderValue) {
        let value = match self {
            XssFilter::Disable => "0",
            XssFilter::Enable => "1",
            XssFilter::ModeBlock => "1; mode=block",
        };

        (
            HeaderName::from_static(Self::HEADER_NAME),
            HeaderValue::from_static(value),
        )
    }
}

pub(crate) struct SecurityLayer {
    inner: Security,
}

impl<S> Layer<S> for SecurityLayer {
    type Service = SecurityMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SecurityMiddleware {
            inner,
            headers: self.inner.0.clone(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct SecurityMiddleware<S> {
    inner: S,
    headers: HeaderMap,
}

impl<S> Service<Request<Body>> for SecurityMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    // `BoxFuture` is a type alias for `Pin<Box<dyn Future + Send + 'a>>`
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        let future = self.inner.call(request);
        let headers = self.headers.clone();
        Box::pin(async move {
            let mut response: Response = future.await?;
            for (key, val) in headers.into_iter() {
                response.headers_mut().insert(key.unwrap(), val);
            }

            Ok(response)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        response::{Html, IntoResponse},
        routing::get,
        test_helpers::*,
        Router,
    };
    use tower::ServiceBuilder;

    async fn handle() -> impl IntoResponse {
        Html("hello world")
    }

    #[tokio::test]
    async fn xss() {
        let policy = Security::new().with(XssFilter::ModeBlock);
        let app = Router::new()
            .route("/", get(handle))
            .layer(ServiceBuilder::new().layer(SecurityLayer { inner: policy }));

        let client = TestClient::new(app);

        let res = client.get("/").send().await;

        let xss = res.headers().get("x-xss-protection");

        assert_eq!(xss, Some(&HeaderValue::from_static("1; mode=block")));
    }
}
