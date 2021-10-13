use crate::BoxError;
use http::{
    header::{HeaderName, HeaderValue},
    Request, StatusCode,
};
use hyper::{Body, Server};
use std::{
    convert::TryFrom,
    net::{SocketAddr, TcpListener},
};
use tower::make::Shared;
use tower_service::Service;

pub(crate) struct TestClient {
    client: reqwest::Client,
    addr: SocketAddr,
}

impl TestClient {
    pub(crate) fn new<S, ResBody>(svc: S) -> Self
    where
        S: Service<Request<Body>, Response = http::Response<ResBody>> + Clone + Send + 'static,
        ResBody: http_body::Body + Send + 'static,
        ResBody::Data: Send,
        ResBody::Error: Into<BoxError>,
        S::Future: Send,
        S::Error: Into<BoxError>,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("Could not bind ephemeral socket");
        let addr = listener.local_addr().unwrap();
        println!("Listening on {}", addr);

        tokio::spawn(async move {
            let server = Server::from_tcp(listener).unwrap().serve(Shared::new(svc));
            server.await.expect("server error");
        });

        TestClient {
            client: reqwest::Client::new(),
            addr,
        }
    }

    pub(crate) fn get(&self, url: &str) -> RequestBuilder {
        RequestBuilder {
            builder: self.client.get(format!("http://{}{}", self.addr, url)),
        }
    }

    pub(crate) fn post(&self, url: &str) -> RequestBuilder {
        RequestBuilder {
            builder: self.client.post(format!("http://{}{}", self.addr, url)),
        }
    }

    pub(crate) fn put(&self, url: &str) -> RequestBuilder {
        RequestBuilder {
            builder: self.client.put(format!("http://{}{}", self.addr, url)),
        }
    }

    pub(crate) fn patch(&self, url: &str) -> RequestBuilder {
        RequestBuilder {
            builder: self.client.patch(format!("http://{}{}", self.addr, url)),
        }
    }
}

pub(crate) struct RequestBuilder {
    builder: reqwest::RequestBuilder,
}

impl RequestBuilder {
    pub(crate) async fn send(self) -> Response {
        Response {
            response: self.builder.send().await.unwrap(),
        }
    }

    pub(crate) fn body(mut self, body: impl Into<reqwest::Body>) -> Self {
        self.builder = self.builder.body(body);
        self
    }

    pub(crate) fn json<T>(mut self, json: &T) -> Self
    where
        T: serde::Serialize,
    {
        self.builder = self.builder.json(json);
        self
    }
    pub(crate) fn header<K, V>(mut self, key: K, value: V) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        self.builder = self.builder.header(key, value);
        self
    }
}

pub(crate) struct Response {
    response: reqwest::Response,
}

impl Response {
    pub(crate) async fn text(self) -> String {
        self.response.text().await.unwrap()
    }

    pub(crate) async fn json<T>(self) -> T
    where
        T: serde::de::DeserializeOwned,
    {
        self.response.json().await.unwrap()
    }

    pub(crate) fn status(&self) -> StatusCode {
        self.response.status()
    }
}
