use async_trait::async_trait;
use axum_core::extract::{FromRequest, RequestParts};
use std::convert::Infallible;

#[derive(Debug, Clone)]
pub struct State<S>(pub S);

#[async_trait]
impl<OuterState, B, InnerState> FromRequest<OuterState, B> for State<InnerState>
where
    B: Send,
    OuterState: Clone + Into<InnerState> + Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<OuterState, B>) -> Result<Self, Self::Rejection> {
        let outer_state = req.state().clone();
        let inner_state = outer_state.into();
        Ok(Self(inner_state))
    }
}
