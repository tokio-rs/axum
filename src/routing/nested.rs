use crate::body::BoxBody;
use http::{Request, Response, Uri};
use std::{
    convert::Infallible,
    task::{Context, Poll},
};
use tower::util::Oneshot;
use tower::ServiceExt;
use tower_service::Service;

/// A [`Service`] that has been nested inside a router at some path.
///
/// Created with [`Router::nest`].
#[derive(Debug, Clone)]
pub(super) struct Nested<S> {
    pub(super) svc: S,
}

impl<B, S> Service<Request<B>> for Nested<S>
where
    S: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible> + Clone,
    B: Send + Sync + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = Oneshot<S, Request<B>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        // strip the prefix from the URI just before calling the inner service
        // such that any surrounding middleware still see the full path
        if let Some(tail) = req.extensions_mut().remove::<NestMatchTail>() {
            UriStack::push(&mut req);
            let new_uri = super::with_path(req.uri(), &tail.0);
            *req.uri_mut() = new_uri;
        }

        self.svc.clone().oneshot(req)
    }
}

pub(crate) struct UriStack(Vec<Uri>);

impl UriStack {
    fn push<B>(req: &mut Request<B>) {
        let uri = req.uri().clone();

        if let Some(stack) = req.extensions_mut().get_mut::<Self>() {
            stack.0.push(uri);
        } else {
            req.extensions_mut().insert(Self(vec![uri]));
        }
    }
}

#[derive(Clone)]
pub(super) struct NestMatchTail(pub(super) String);

#[test]
fn traits() {
    use crate::tests::*;

    assert_send::<Nested<()>>();
    assert_sync::<Nested<()>>();
}
