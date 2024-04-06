//! Axum Test Client
//!
//! ```rust
//! use axum::Router;
//! use axum::http::StatusCode;
//! use axum::routing::get;
//! use axum::test_client::TestClient;
//!
//! let async_block = async {
//!     // you can replace this Router with your own app
//!     let app = Router::new().route("/", get(|| async {}));
//!
//!     // initiate the TestClient with the previous declared Router
//!     let client = TestClient::new(app);
//!
//!     let res = client.get("/").await;
//!     assert_eq!(res.status(), StatusCode::OK);
//! };
//!
//! // Create a runtime for executing the async block. This runtime is local
//! // to the main function and does not require any global setup.
//! let runtime = tokio::runtime::Builder::new_current_thread()
//!     .enable_all()
//!     .build()
//!     .unwrap();
//!
//! // Use the local runtime to block on the async block.
//! runtime.block_on(async_block);
//! ```

use super::{serve, Request, Response};
use bytes::Bytes;
use futures_util::future::BoxFuture;
use http::{
    header::{HeaderName, HeaderValue},
    StatusCode,
};
use std::{convert::Infallible, future::IntoFuture, net::SocketAddr};
use tokio::net::TcpListener;
use tower::make::Shared;
use tower_service::Service;

pub(crate) fn spawn_service<S>(svc: S) -> SocketAddr
where
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
{
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    std_listener.set_nonblocking(true).unwrap();
    let listener = TcpListener::from_std(std_listener).unwrap();

    let addr = listener.local_addr().unwrap();
    #[cfg(feature = "tracing")]
    tracing::info!("Listening on {addr}");

    tokio::spawn(async move {
        serve(listener, Shared::new(svc))
            .await
            .expect("server error")
    });

    addr
}

/// Test client to Axum servers.
#[derive(Debug)]
pub struct TestClient {
    client: reqwest::Client,
    addr: SocketAddr,
}

impl TestClient {
    /// Create a new test client.
    pub fn new<S>(svc: S) -> Self
    where
        S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
        S::Future: Send,
    {
        let addr = spawn_service(svc);

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();

        TestClient { client, addr }
    }

    /// Create a GET request.
    pub fn get(&self, url: &str) -> RequestBuilder {
        RequestBuilder {
            builder: self.client.get(format!("http://{}{}", self.addr, url)),
        }
    }

    /// Create a HEAD request.
    pub fn head(&self, url: &str) -> RequestBuilder {
        RequestBuilder {
            builder: self.client.head(format!("http://{}{}", self.addr, url)),
        }
    }

    /// Create a POST request.
    pub fn post(&self, url: &str) -> RequestBuilder {
        RequestBuilder {
            builder: self.client.post(format!("http://{}{}", self.addr, url)),
        }
    }

    /// Create a PUT request.
    pub fn put(&self, url: &str) -> RequestBuilder {
        RequestBuilder {
            builder: self.client.put(format!("http://{}{}", self.addr, url)),
        }
    }

    /// Create a PATCH request.
    pub fn patch(&self, url: &str) -> RequestBuilder {
        RequestBuilder {
            builder: self.client.patch(format!("http://{}{}", self.addr, url)),
        }
    }
}

/// Builder for test requests.
#[derive(Debug)]
pub struct RequestBuilder {
    builder: reqwest::RequestBuilder,
}

impl RequestBuilder {
    /// Set the request body.
    pub fn body(mut self, body: impl Into<reqwest::Body>) -> Self {
        self.builder = self.builder.body(body);
        self
    }

    /// Set the request JSON body.
    pub fn json<T>(mut self, json: &T) -> Self
    where
        T: serde::Serialize,
    {
        self.builder = self.builder.json(json);
        self
    }

    /// Set a request header.
    pub fn header<K, V>(mut self, key: K, value: V) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        self.builder = self.builder.header(key, value);
        self
    }

    /// Set a request multipart form.
    pub fn multipart(mut self, form: reqwest::multipart::Form) -> Self {
        self.builder = self.builder.multipart(form);
        self
    }
}

impl IntoFuture for RequestBuilder {
    type Output = TestResponse;
    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async {
            TestResponse {
                response: self.builder.send().await.unwrap(),
            }
        })
    }
}

/// Test response.
#[derive(Debug)]
pub struct TestResponse {
    response: reqwest::Response,
}

impl TestResponse {
    /// Get the response body as bytes.
    pub async fn bytes(self) -> Bytes {
        self.response.bytes().await.unwrap()
    }

    /// Get the response body as text.
    pub async fn text(self) -> String {
        self.response.text().await.unwrap()
    }

    /// Get the response body as JSON.
    pub async fn json<T>(self) -> T
    where
        T: serde::de::DeserializeOwned,
    {
        self.response.json().await.unwrap()
    }

    /// Get the response status.
    pub fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.response.status().as_u16()).unwrap()
    }

    /// Get the response headers.
    pub fn headers(&self) -> http::HeaderMap {
        self.response.headers().clone()
    }

    /// Get the response in chunks.
    pub async fn chunk(&mut self) -> Option<Bytes> {
        self.response.chunk().await.unwrap()
    }

    /// Get the response in chunks as text.
    pub async fn chunk_text(&mut self) -> Option<String> {
        let chunk = self.chunk().await?;
        Some(String::from_utf8(chunk.to_vec()).unwrap())
    }
}
