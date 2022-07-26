use async_trait::async_trait;
use axum_core::extract::{FromRequest, RequestParts};
use std::{
    convert::Infallible,
    ops::{Deref, DerefMut},
};

#[derive(Debug, Default, Clone, Copy)]
pub struct State<S>(pub S);

#[async_trait]
impl<B, OuterState, InnerState> FromRequest<B, OuterState> for State<InnerState>
where
    B: Send,
    OuterState: Clone + Into<InnerState> + Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<B, OuterState>) -> Result<Self, Self::Rejection> {
        let outer_state = req.state().clone();
        let inner_state = outer_state.into();
        Ok(Self(inner_state))
    }
}

impl<S> Deref for State<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S> DerefMut for State<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
