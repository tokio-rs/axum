use axum::async_trait;
use axum::extract::{FromRequest, RequestParts};
use axum::response::IntoResponse;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Copy, Default)]
pub struct WithRejection<E, R>(pub E, pub PhantomData<R>);

impl<E, R> WithRejection<E, R> {
    fn into_inner(self) -> E {
        self.0
    }
}

impl<E, R> Deref for WithRejection<E, R> {
    type Target = E;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<E, R> DerefMut for WithRejection<E, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[async_trait]
impl<B, E, R> FromRequest<B> for WithRejection<E, R>
where
    B: Send,
    E: FromRequest<B>,
    R: From<E::Rejection> + IntoResponse,
{
    type Rejection = R;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let extractor = req.extract::<E>().await?;
        Ok(WithRejection(extractor, PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use axum::http::Request;
    use axum::response::Response;

    use super::*;

    #[tokio::test]
    async fn extractor_rejection_is_transformed() {
        struct TestExtractor;
        struct TestRejection;

        #[async_trait]
        impl<B: Send> FromRequest<B> for TestExtractor {
            type Rejection = ();

            async fn from_request(_: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
                Err(())
            }
        }

        impl IntoResponse for TestRejection {
            fn into_response(self) -> Response {
                ().into_response()
            }
        }

        impl From<()> for TestRejection {
            fn from(_: ()) -> Self {
                TestRejection
            }
        }

        let mut req = RequestParts::new(Request::new(()));

        let result = req
            .extract::<WithRejection<TestExtractor, TestRejection>>()
            .await;

        assert!(matches!(result, Err(TestRejection)))
    }
}
