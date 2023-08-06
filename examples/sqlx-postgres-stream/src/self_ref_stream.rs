use std::pin::Pin;

use futures::{
    stream::BoxStream,
    task::{Context, Poll},
    Stream,
};

#[ouroboros::self_referencing]
pub struct SelfRefStream<Params: 'static, Item> {
    params: Params,
    #[borrows(params)]
    #[covariant]
    inner: BoxStream<'this, Item>,
}

impl<Params: 'static, Item> SelfRefStream<Params, Item> {
    #[inline]
    pub fn build(
        params: Params,
        inner_builder: impl for<'this> FnOnce(&'this Params) -> BoxStream<'this, Item>,
    ) -> Self {
        SelfRefStreamBuilder {
            params,
            inner_builder,
        }
        .build()
    }
}

impl<Params: 'static, Item> Stream for SelfRefStream<Params, Item> {
    type Item = Item;

    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.with_inner_mut(|s| s.as_mut().poll_next(cx))
    }
}
