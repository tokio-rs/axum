use crate::body::Body;
use crate::extract::{DefaultBodyLimitKind, FromRequest, FromRequestParts, Request};
use http::request::Parts;
use std::future::Future;

mod sealed {
    pub trait Sealed {}
    impl Sealed for http::Request<crate::body::Body> {}
}

/// Extension trait that adds additional methods to [`Request`].
pub trait RequestExt: sealed::Sealed + Sized {
    /// Apply an extractor to this `Request`.
    ///
    /// This is just a convenience for `E::from_request(req, &())`.
    ///
    /// Note this consumes the request. Use [`RequestExt::extract_parts`] if you're not extracting
    /// the body and don't want to consume the request.
    ///
    /// # Example
    ///
    /// ```
    /// use axum::{
    ///     extract::{Request, FromRequest},
    ///     body::Body,
    ///     http::{header::CONTENT_TYPE, StatusCode},
    ///     response::{IntoResponse, Response},
    ///     Form, Json, RequestExt,
    /// };
    ///
    /// struct FormOrJson<T>(T);
    ///
    /// impl<S, T> FromRequest<S> for FormOrJson<T>
    /// where
    ///     Json<T>: FromRequest<()>,
    ///     Form<T>: FromRequest<()>,
    ///     T: 'static,
    ///     S: Send + Sync,
    /// {
    ///     type Rejection = Response;
    ///
    ///     async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
    ///         let content_type = req
    ///             .headers()
    ///             .get(CONTENT_TYPE)
    ///             .and_then(|value| value.to_str().ok())
    ///             .ok_or_else(|| StatusCode::BAD_REQUEST.into_response())?;
    ///
    ///         if content_type.starts_with("application/json") {
    ///             let Json(payload) = req
    ///                 .extract::<Json<T>, _>()
    ///                 .await
    ///                 .map_err(|err| err.into_response())?;
    ///
    ///             Ok(Self(payload))
    ///         } else if content_type.starts_with("application/x-www-form-urlencoded") {
    ///             let Form(payload) = req
    ///                 .extract::<Form<T>, _>()
    ///                 .await
    ///                 .map_err(|err| err.into_response())?;
    ///
    ///             Ok(Self(payload))
    ///         } else {
    ///             Err(StatusCode::BAD_REQUEST.into_response())
    ///         }
    ///     }
    /// }
    /// ```
    fn extract<E, M>(self) -> impl Future<Output = Result<E, E::Rejection>> + Send
    where
        E: FromRequest<(), M> + 'static,
        M: 'static;

    /// Apply an extractor that requires some state to this `Request`.
    ///
    /// This is just a convenience for `E::from_request(req, state)`.
    ///
    /// Note this consumes the request. Use [`RequestExt::extract_parts_with_state`] if you're not
    /// extracting the body and don't want to consume the request.
    ///
    /// # Example
    ///
    /// ```
    /// use axum::{
    ///     body::Body,
    ///     extract::{Request, FromRef, FromRequest},
    ///     RequestExt,
    /// };
    ///
    /// struct MyExtractor {
    ///     requires_state: RequiresState,
    /// }
    ///
    /// impl<S> FromRequest<S> for MyExtractor
    /// where
    ///     String: FromRef<S>,
    ///     S: Send + Sync,
    /// {
    ///     type Rejection = std::convert::Infallible;
    ///
    ///     async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
    ///         let requires_state = req.extract_with_state::<RequiresState, _, _>(state).await?;
    ///
    ///         Ok(Self { requires_state })
    ///     }
    /// }
    ///
    /// // some extractor that consumes the request body and requires state
    /// struct RequiresState { /* ... */ }
    ///
    /// impl<S> FromRequest<S> for RequiresState
    /// where
    ///     String: FromRef<S>,
    ///     S: Send + Sync,
    /// {
    ///     // ...
    ///     # type Rejection = std::convert::Infallible;
    ///     # async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
    ///     #     todo!()
    ///     # }
    /// }
    /// ```
    fn extract_with_state<E, S, M>(
        self,
        state: &S,
    ) -> impl Future<Output = Result<E, E::Rejection>> + Send
    where
        E: FromRequest<S, M> + 'static,
        S: Send + Sync;

    /// Apply a parts extractor to this `Request`.
    ///
    /// This is just a convenience for `E::from_request_parts(parts, state)`.
    ///
    /// If extraction is cancelled, the request retains its parts, including any
    /// changes the extractor made before cancellation.
    ///
    /// # Example
    ///
    /// ```
    /// use axum::{
    ///     extract::{Path, Request, FromRequest},
    ///     response::{IntoResponse, Response},
    ///     body::Body,
    ///     Json, RequestExt,
    /// };
    /// use axum_extra::{
    ///     TypedHeader,
    ///     headers::{authorization::Bearer, Authorization},
    /// };
    /// use std::collections::HashMap;
    ///
    /// struct MyExtractor<T> {
    ///     path_params: HashMap<String, String>,
    ///     payload: T,
    /// }
    ///
    /// impl<S, T> FromRequest<S> for MyExtractor<T>
    /// where
    ///     S: Send + Sync,
    ///     Json<T>: FromRequest<()>,
    ///     T: 'static,
    /// {
    ///     type Rejection = Response;
    ///
    ///     async fn from_request(mut req: Request, _state: &S) -> Result<Self, Self::Rejection> {
    ///         let path_params = req
    ///             .extract_parts::<Path<_>>()
    ///             .await
    ///             .map(|Path(path_params)| path_params)
    ///             .map_err(|err| err.into_response())?;
    ///
    ///         let Json(payload) = req
    ///             .extract::<Json<T>, _>()
    ///             .await
    ///             .map_err(|err| err.into_response())?;
    ///
    ///         Ok(Self { path_params, payload })
    ///     }
    /// }
    /// ```
    fn extract_parts<E>(&mut self) -> impl Future<Output = Result<E, E::Rejection>> + Send
    where
        E: FromRequestParts<()> + 'static;

    /// Apply a parts extractor that requires some state to this `Request`.
    ///
    /// This is just a convenience for `E::from_request_parts(parts, state)`.
    ///
    /// If extraction is cancelled, the request retains its parts, including any
    /// changes the extractor made before cancellation.
    ///
    /// # Example
    ///
    /// ```
    /// use axum::{
    ///     extract::{Request, FromRef, FromRequest, FromRequestParts},
    ///     http::request::Parts,
    ///     response::{IntoResponse, Response},
    ///     body::Body,
    ///     Json, RequestExt,
    /// };
    ///
    /// struct MyExtractor<T> {
    ///     requires_state: RequiresState,
    ///     payload: T,
    /// }
    ///
    /// impl<S, T> FromRequest<S> for MyExtractor<T>
    /// where
    ///     String: FromRef<S>,
    ///     Json<T>: FromRequest<()>,
    ///     T: 'static,
    ///     S: Send + Sync,
    /// {
    ///     type Rejection = Response;
    ///
    ///     async fn from_request(mut req: Request, state: &S) -> Result<Self, Self::Rejection> {
    ///         let requires_state = req
    ///             .extract_parts_with_state::<RequiresState, _>(state)
    ///             .await
    ///             .map_err(|err| err.into_response())?;
    ///
    ///         let Json(payload) = req
    ///             .extract::<Json<T>, _>()
    ///             .await
    ///             .map_err(|err| err.into_response())?;
    ///
    ///         Ok(Self {
    ///             requires_state,
    ///             payload,
    ///         })
    ///     }
    /// }
    ///
    /// struct RequiresState {}
    ///
    /// impl<S> FromRequestParts<S> for RequiresState
    /// where
    ///     String: FromRef<S>,
    ///     S: Send + Sync,
    /// {
    ///     // ...
    ///     # type Rejection = std::convert::Infallible;
    ///     # async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
    ///     #     todo!()
    ///     # }
    /// }
    /// ```
    fn extract_parts_with_state<'a, E, S>(
        &'a mut self,
        state: &'a S,
    ) -> impl Future<Output = Result<E, E::Rejection>> + Send + 'a
    where
        E: FromRequestParts<S> + 'static,
        S: Send + Sync;

    /// Apply the [default body limit](crate::extract::DefaultBodyLimit).
    ///
    /// If it is disabled, the request is returned as-is.
    fn with_limited_body(self) -> Request;

    /// Consumes the request, returning the body wrapped in [`http_body_util::Limited`] if a
    /// [default limit](crate::extract::DefaultBodyLimit) is in place, or not wrapped if the
    /// default limit is disabled.
    fn into_limited_body(self) -> Body;
}

// Restore the parts even when the extraction future is dropped before completion.
struct RequestPartsGuard<'a> {
    request: &'a mut Request,
    parts: Parts,
}

impl Drop for RequestPartsGuard<'_> {
    fn drop(&mut self) {
        std::mem::swap(self.request.version_mut(), &mut self.parts.version);
        std::mem::swap(self.request.method_mut(), &mut self.parts.method);
        std::mem::swap(self.request.uri_mut(), &mut self.parts.uri);
        std::mem::swap(self.request.headers_mut(), &mut self.parts.headers);
        std::mem::swap(self.request.extensions_mut(), &mut self.parts.extensions);
    }
}

impl RequestExt for Request {
    fn extract<E, M>(self) -> impl Future<Output = Result<E, E::Rejection>> + Send
    where
        E: FromRequest<(), M> + 'static,
        M: 'static,
    {
        self.extract_with_state(&())
    }

    fn extract_with_state<E, S, M>(
        self,
        state: &S,
    ) -> impl Future<Output = Result<E, E::Rejection>> + Send
    where
        E: FromRequest<S, M> + 'static,
        S: Send + Sync,
    {
        E::from_request(self, state)
    }

    fn extract_parts<E>(&mut self) -> impl Future<Output = Result<E, E::Rejection>> + Send
    where
        E: FromRequestParts<()> + 'static,
    {
        self.extract_parts_with_state(&())
    }

    async fn extract_parts_with_state<'a, E, S>(
        &'a mut self,
        state: &'a S,
    ) -> Result<E, E::Rejection>
    where
        E: FromRequestParts<S> + 'static,
        S: Send + Sync,
    {
        let mut req = Request::new(());
        *req.version_mut() = self.version();
        *req.method_mut() = self.method().clone();
        *req.uri_mut() = self.uri().clone();
        *req.headers_mut() = std::mem::take(self.headers_mut());
        *req.extensions_mut() = std::mem::take(self.extensions_mut());
        let (parts, ()) = req.into_parts();
        let mut guard = RequestPartsGuard {
            request: self,
            parts,
        };

        E::from_request_parts(&mut guard.parts, state).await
    }

    fn with_limited_body(self) -> Request {
        // update docs in `axum-core/src/extract/default_body_limit.rs` and
        // `axum/src/docs/extract.md` if this changes
        const DEFAULT_LIMIT: usize = 2_097_152; // 2 mb

        match self.extensions().get::<DefaultBodyLimitKind>().copied() {
            Some(DefaultBodyLimitKind::Disable) => self,
            Some(DefaultBodyLimitKind::Limit(limit)) => {
                self.map(|b| Body::new(http_body_util::Limited::new(b, limit)))
            }
            None => self.map(|b| Body::new(http_body_util::Limited::new(b, DEFAULT_LIMIT))),
        }
    }

    fn into_limited_body(self) -> Body {
        self.with_limited_body().into_body()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ext_traits::tests::{RequiresState, State},
        extract::FromRef,
    };
    use http::Method;

    #[tokio::test]
    async fn extract_without_state() {
        let req = Request::new(Body::empty());

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
        let req = Request::new(Body::empty());

        let state = "state".to_owned();

        let State(extracted_state): State<String> = req.extract_with_state(&state).await.unwrap();

        assert_eq!(extracted_state, state);
    }

    #[tokio::test]
    async fn extract_parts_without_state() {
        let mut req = Request::builder()
            .header("x-foo", "foo")
            .body(Body::empty())
            .unwrap();

        let method: Method = req.extract_parts().await.unwrap();

        assert_eq!(method, Method::GET);
        assert_eq!(req.headers()["x-foo"], "foo");
    }

    #[tokio::test]
    async fn extract_parts_with_state() {
        let mut req = Request::builder()
            .header("x-foo", "foo")
            .body(Body::empty())
            .unwrap();

        let state = "state".to_owned();

        let State(extracted_state): State<String> =
            req.extract_parts_with_state(&state).await.unwrap();

        assert_eq!(extracted_state, state);
        assert_eq!(req.headers()["x-foo"], "foo");
    }

    struct PendingExtractor;

    impl FromRequestParts<()> for PendingExtractor {
        type Rejection = std::convert::Infallible;

        async fn from_request_parts(
            _parts: &mut http::request::Parts,
            _state: &(),
        ) -> Result<Self, Self::Rejection> {
            std::future::pending().await
        }
    }

    #[tokio::test]
    async fn extract_parts_cancellation_preserves_request() {
        let mut req = Request::builder()
            .method(Method::POST)
            .uri("/original")
            .version(http::Version::HTTP_2)
            .header("x-original", "value")
            .extension(42_u32)
            .body(Body::from("payload"))
            .unwrap();

        {
            let mut extraction = Box::pin(req.extract_parts::<PendingExtractor>());
            std::future::poll_fn(|cx| {
                assert!(extraction.as_mut().poll(cx).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
        }

        assert_eq!(req.method(), Method::POST);
        assert_eq!(req.uri(), "/original");
        assert_eq!(req.version(), http::Version::HTTP_2);
        assert_eq!(req.headers()["x-original"], "value");
        assert_eq!(req.extensions().get::<u32>(), Some(&42));
        assert_eq!(req.extract::<String, _>().await.unwrap(), "payload");
    }

    enum ExtractionMode {
        Pending,
        Success,
        Reject,
    }

    #[derive(Debug)]
    struct ModifyParts;

    impl FromRequestParts<ExtractionMode> for ModifyParts {
        type Rejection = http::StatusCode;

        async fn from_request_parts(
            parts: &mut http::request::Parts,
            state: &ExtractionMode,
        ) -> Result<Self, Self::Rejection> {
            parts.method = Method::PATCH;
            parts.uri = "/changed".parse().unwrap();
            parts.version = http::Version::HTTP_11;
            parts.headers.insert("x-added", "added".parse().unwrap());
            parts.extensions.insert(42_u32);

            match state {
                ExtractionMode::Pending => std::future::pending().await,
                ExtractionMode::Success => Ok(Self),
                ExtractionMode::Reject => Err(http::StatusCode::BAD_REQUEST),
            }
        }
    }

    async fn assert_modified_request(req: Request) {
        assert_eq!(req.method(), Method::PATCH);
        assert_eq!(req.uri(), "/changed");
        assert_eq!(req.version(), http::Version::HTTP_11);
        assert_eq!(req.headers()["x-original"], "value");
        assert_eq!(req.headers()["x-added"], "added");
        assert_eq!(req.extensions().get::<u32>(), Some(&42));
        assert_eq!(req.extract::<String, _>().await.unwrap(), "payload");
    }

    #[tokio::test]
    async fn extract_parts_with_state_cancellation_preserves_changes() {
        let mut req = Request::builder()
            .header("x-original", "value")
            .body(Body::from("payload"))
            .unwrap();

        {
            let mut extraction =
                Box::pin(req.extract_parts_with_state::<ModifyParts, _>(&ExtractionMode::Pending));
            std::future::poll_fn(|cx| {
                assert!(extraction.as_mut().poll(cx).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
        }

        assert_modified_request(req).await;
    }

    #[tokio::test]
    async fn extract_parts_with_state_preserves_changes_on_completion() {
        for mode in [ExtractionMode::Success, ExtractionMode::Reject] {
            let mut req = Request::builder()
                .header("x-original", "value")
                .body(Body::from("payload"))
                .unwrap();

            let result = req.extract_parts_with_state::<ModifyParts, _>(&mode).await;
            match mode {
                ExtractionMode::Success => assert!(result.is_ok()),
                ExtractionMode::Reject => {
                    assert_eq!(result.unwrap_err(), http::StatusCode::BAD_REQUEST);
                }
                ExtractionMode::Pending => unreachable!(),
            }
            assert_modified_request(req).await;
        }
    }

    // this stuff just needs to compile
    #[allow(dead_code)]
    struct WorksForCustomExtractor {
        method: Method,
        from_state: String,
        body: String,
    }

    impl<S> FromRequest<S> for WorksForCustomExtractor
    where
        S: Send + Sync,
        String: FromRef<S> + FromRequest<()>,
    {
        type Rejection = <String as FromRequest<()>>::Rejection;

        async fn from_request(mut req: Request, state: &S) -> Result<Self, Self::Rejection> {
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
