pub(crate) mod request;
pub(crate) mod request_parts;
pub(crate) mod service;

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use async_trait::async_trait;
    use axum_core::extract::{FromRef, FromRequestParts};
    use http::request::Parts;

    // some extractor that requires the state, such as `SignedCookieJar`
    pub(crate) struct RequiresState(pub(crate) String);

    #[async_trait]
    impl<S> FromRequestParts<S> for RequiresState
    where
        S: Send + Sync,
        String: FromRef<S>,
    {
        type Rejection = Infallible;

        async fn from_request_parts(
            _parts: &mut Parts,
            state: &S,
        ) -> Result<Self, Self::Rejection> {
            Ok(Self(String::from_ref(state)))
        }
    }
}
