#[cfg(test)]
mod tests {
    use crate::{extract::Extension, routing::get, test_helpers::*, Router};
    use http::{Method, StatusCode};

    #[crate::test]
    async fn extract_request_parts() {
        #[derive(Clone)]
        struct Ext;

        async fn handler(parts: http::request::Parts) {
            assert_eq!(parts.method, Method::GET);
            assert_eq!(parts.uri, "/");
            assert_eq!(parts.version, http::Version::HTTP_11);
            assert_eq!(parts.headers["x-foo"], "123");
            parts.extensions.get::<Ext>().unwrap();
        }

        let client = TestClient::new(Router::new().route("/", get(handler)).layer(Extension(Ext)));

        let res = client.get("/").header("x-foo", "123").await;
        assert_eq!(res.status(), StatusCode::OK);
    }
}
