use async_trait::async_trait;
use axum_core::extract::{FromRequest, RequestParts};
use http::Request;

/// Extension trait that adds additional methods to [`Request`].
///
/// This trait is sealed so it cannot be implemented outside of axum.
#[async_trait]
pub trait RequestExt<B>: sealed::Sealed {
    /// Extract a value from the request.
    ///
    /// # Example
    ///
    /// This can be used to run extractors from middleware:
    ///
    /// ```
    /// use axum::{
    ///     Router,
    ///     RequestExt,
    ///     extract::{Query, rejection::QueryRejection},
    ///     http::Request,
    ///     middleware::{self, Next},
    ///     response::Response,
    /// };
    /// use std::collections::HashMap;
    ///
    /// async fn my_middleware<B>(
    ///     request: Request<B>,
    ///     next: Next<B>
    /// ) -> Result<Response, QueryRejection>
    /// where
    ///     B: Send
    /// {
    ///     let (query_params, request) = request
    ///         .extract::<Query<HashMap<String, String>>>()
    ///         .await?;
    ///
    ///     // do something with `query_params`...
    ///
    ///     Ok(next.run(request).await)
    /// }
    ///
    /// let app = Router::new().layer(middleware::from_fn(my_middleware));
    /// # let _: Router = app;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the request body is extracted. Use [`RequestExt::extract_body`] instead if you
    /// wish to extract the request body.
    async fn extract<T>(self) -> Result<(T, Request<B>), T::Rejection>
    where
        T: FromRequest<B>;

    /// Extract a value (possibly the body) from the request.
    ///
    /// This differs from [`RequestExt::extract`] in that it does not panic if the request body is
    /// extracted. Instead it returns the request parts so you can rebuild the request if
    /// necessary.
    ///
    /// # Example
    ///
    /// This can be used to run extractors from middleware:
    ///
    /// ```
    /// use axum::{
    ///     Router,
    ///     RequestExt,
    ///     extract::rejection::BytesRejection,
    ///     http::Request,
    ///     middleware::{self, Next},
    ///     response::Response,
    ///     body::{Body, Bytes},
    /// };
    ///
    /// async fn my_middleware(
    ///     request: Request<Body>,
    ///     next: Next<Body>
    /// ) -> Result<Response, BytesRejection> {
    ///     let (bytes, parts) = request.extract_body::<Bytes>().await?;
    ///
    ///     // do something with `bytes`...
    ///
    ///     // rebuild the request so can we pass it to `next`
    ///     let request = Request::from_parts(parts, Body::from(bytes));
    ///
    ///     Ok(next.run(request).await)
    /// }
    ///
    /// let app = Router::new().layer(middleware::from_fn(my_middleware));
    /// # let _: Router = app;
    /// ```
    async fn extract_body<T>(self) -> Result<(T, http::request::Parts), T::Rejection>
    where
        T: FromRequest<B>;
}

#[async_trait]
impl<B> RequestExt<B> for Request<B>
where
    B: Send,
{
    async fn extract<T>(self) -> Result<(T, Request<B>), T::Rejection>
    where
        T: FromRequest<B>,
    {
        let mut parts = RequestParts::new(self);
        let t = T::from_request(&mut parts).await?;
        let req = parts.try_into_request().expect(
            "`RequestExt::extract` does not support extracting the request body. \
            Use `RequestExt::extract_body` instead",
        );
        Ok((t, req))
    }

    async fn extract_body<T>(self) -> Result<(T, http::request::Parts), T::Rejection>
    where
        T: FromRequest<B>,
    {
        let mut parts = RequestParts::new(self);
        let t = T::from_request(&mut parts).await?;
        let parts = parts.into_parts();
        Ok((t, parts))
    }
}

impl<B> sealed::Sealed for Request<B> {}

mod sealed {
    pub trait Sealed {}
}
