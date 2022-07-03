use super::{FromRequest, RequestParts};
use async_trait::async_trait;
use std::convert::Infallible;

/// TODO(david): docs
// TODO(david): document how to extract this from middleware
#[derive(Clone, Copy, Debug, Default)]
pub struct State<S>(pub S);

#[async_trait]
impl<S, B> FromRequest<S, B> for State<S>
where
    B: Send,
    S: Clone + Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<S, B>) -> Result<Self, Self::Rejection> {
        Ok(Self(req.state().clone()))
    }
}
