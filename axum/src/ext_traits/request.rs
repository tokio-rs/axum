use async_trait::async_trait;
use axum_core::extract::{FromRequest, FromRequestParts};
use http::Request;

mod sealed {
    pub trait Sealed<B> {}
    impl<B> Sealed<B> for http::Request<B> {}
}

/// Extension trait that adds additional methods to [`Request`].
#[async_trait]
pub trait RequestExt<B>: sealed::Sealed<B> + Sized {
    /// Apply an extractor to this `Request`.
    ///
    /// This is just a convenience for `E::from_request(req, &())`.
    ///
    /// Note this consumes the request. Use [`RequestExt::extract_parts`] if you're not extracting
    /// the body and don't want to consume the request.
    async fn extract<E, M>(self) -> Result<E, E::Rejection>
    where
        E: FromRequest<(), B, M>;

    /// Apply an extractor that requires some state to this `Request`.
    ///
    /// This is just a convenience for `E::from_request(req, state)`.
    ///
    /// Note this consumes the request. Use [`RequestExt::extract_parts_with_state`] if you're not
    /// extracting the body and don't want to consume the request.
    async fn extract_with_state<E, S, M>(self, state: &S) -> Result<E, E::Rejection>
    where
        E: FromRequest<S, B, M>,
        S: Send + Sync;

    /// Apply a parts extractor to this `Request`.
    ///
    /// This is just a convenience for `E::from_request_parts(parts, state)`.
    async fn extract_parts<E>(&mut self) -> Result<E, E::Rejection>
    where
        E: FromRequestParts<()>;

    /// Apply a parts extractor that requires some state to this `Request`.
    ///
    /// This is just a convenience for `E::from_request_parts(parts, state)`.
    async fn extract_parts_with_state<E, S>(&mut self, state: &S) -> Result<E, E::Rejection>
    where
        E: FromRequestParts<S>,
        S: Send + Sync;
}

#[async_trait]
impl<B> RequestExt<B> for Request<B>
where
    B: Send,
{
    async fn extract<E, M>(self) -> Result<E, E::Rejection>
    where
        E: FromRequest<(), B, M>,
    {
        self.extract_with_state(&()).await
    }

    async fn extract_with_state<E, S, M>(self, state: &S) -> Result<E, E::Rejection>
    where
        E: FromRequest<S, B, M>,
        S: Send + Sync,
    {
        E::from_request(self, state).await
    }

    async fn extract_parts<E>(&mut self) -> Result<E, E::Rejection>
    where
        E: FromRequestParts<()>,
    {
        self.extract_parts_with_state(&()).await
    }

    async fn extract_parts_with_state<E, S>(&mut self, state: &S) -> Result<E, E::Rejection>
    where
        E: FromRequestParts<S>,
        S: Send + Sync,
    {
        let mut req = Request::new(());
        *req.version_mut() = self.version();
        *req.method_mut() = self.method().clone();
        *req.uri_mut() = self.uri().clone();
        *req.headers_mut() = std::mem::take(self.headers_mut());
        *req.extensions_mut() = std::mem::take(self.extensions_mut());
        let (mut parts, _) = req.into_parts();

        let result = E::from_request_parts(&mut parts, state).await;

        *self.version_mut() = parts.version;
        *self.method_mut() = parts.method.clone();
        *self.uri_mut() = parts.uri.clone();
        *self.headers_mut() = std::mem::take(&mut parts.headers);
        *self.extensions_mut() = std::mem::take(&mut parts.extensions);

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ext_traits::tests::RequiresState, extract::State};
    use axum_core::extract::FromRef;
    use http::Method;
    use hyper::Body;

    #[tokio::test]
    async fn extract_without_state() {
        let req = Request::new(());

        let method: Method = req.extract().await.unwrap();

        assert_eq!(method, Method::GET);
    }

    #[tokio::test]
    async fn extract_body_without_state() {
        let req = Request::new(Body::from("foobar"));

        let body: String = req.extract().await.unwrap();

        assert_eq!(body, "foobar");
    }

    #[tokio::test]
    async fn extract_with_state() {
        let req = Request::new(());

        let state = "state".to_owned();

        let State(extracted_state): State<String> = req.extract_with_state(&state).await.unwrap();

        assert_eq!(extracted_state, state);
    }

    #[tokio::test]
    async fn extract_parts_without_state() {
        let mut req = Request::builder().header("x-foo", "foo").body(()).unwrap();

        let method: Method = req.extract_parts().await.unwrap();

        assert_eq!(method, Method::GET);
        assert_eq!(req.headers()["x-foo"], "foo");
    }

    #[tokio::test]
    async fn extract_parts_with_state() {
        let mut req = Request::builder().header("x-foo", "foo").body(()).unwrap();

        let state = "state".to_owned();

        let State(extracted_state): State<String> =
            req.extract_parts_with_state(&state).await.unwrap();

        assert_eq!(extracted_state, state);
        assert_eq!(req.headers()["x-foo"], "foo");
    }

    // this stuff just needs to compile
    #[allow(dead_code)]
    struct WorksForCustomExtractor {
        method: Method,
        from_state: String,
        body: String,
    }

    #[async_trait]
    impl<S, B> FromRequest<S, B> for WorksForCustomExtractor
    where
        S: Send + Sync,
        B: Send + 'static,
        String: FromRef<S> + FromRequest<(), B>,
    {
        type Rejection = <String as FromRequest<(), B>>::Rejection;

        async fn from_request(mut req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
            let RequiresState(from_state) = req.extract_parts_with_state(state).await.unwrap();
            let method = req.extract_parts().await.unwrap();
            let body = req.extract().await?;

            Ok(Self {
                method,
                from_state,
                body,
            })
        }
    }
}
