use futures_util::ready;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::sync::{mpsc, oneshot, OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::PollSemaphore;
use tower::{Service, ServiceExt};

/// A version of [`tower::buffer::Buffer`] which panicks on channel related errors, thus keeping
/// the error type of the service.
pub(crate) struct MpscBuffer<S, R>
where
    S: Service<R>,
{
    tx: mpsc::UnboundedSender<Msg<S, R>>,
    semaphore: PollSemaphore,
    permit: Option<OwnedSemaphorePermit>,
}

impl<S, R> Clone for MpscBuffer<S, R>
where
    S: Service<R>,
{
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            semaphore: self.semaphore.clone(),
            permit: None,
        }
    }
}

impl<S, R> MpscBuffer<S, R>
where
    S: Service<R>,
{
    pub(crate) fn new(svc: S) -> Self
    where
        S: Send + 'static,
        R: Send + 'static,
        S::Error: Send + 'static,
        S::Future: Send + 'static,
    {
        let (tx, rx) = mpsc::unbounded_channel::<Msg<S, R>>();
        let semaphore = PollSemaphore::new(Arc::new(Semaphore::new(1024)));

        tokio::spawn(run_worker(svc, rx));

        Self {
            tx,
            semaphore,
            permit: None,
        }
    }
}

async fn run_worker<S, R>(mut svc: S, mut rx: mpsc::UnboundedReceiver<Msg<S, R>>)
where
    S: Service<R>,
{
    while let Some((req, reply_tx)) = rx.recv().await {
        match svc.ready().await {
            Ok(svc) => {
                let future = svc.call(req);
                let _ = reply_tx.send(WorkerReply::Future(future));
            }
            Err(err) => {
                let _ = reply_tx.send(WorkerReply::Error(err));
            }
        }
    }
}

type Msg<S, R> = (
    R,
    oneshot::Sender<WorkerReply<<S as Service<R>>::Future, <S as Service<R>>::Error>>,
);

enum WorkerReply<F, E> {
    Future(F),
    Error(E),
}

impl<S, R> Service<R> for MpscBuffer<S, R>
where
    S: Service<R>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future, S::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.permit.is_some() {
            return Poll::Ready(Ok(()));
        }

        let permit = ready!(self.semaphore.poll_acquire(cx))
            .expect("buffer semaphore closed. This is a bug in axum and should never happen. Please file an issue");

        self.permit = Some(permit);

        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: R) -> Self::Future {
        let permit = self
            .permit
            .take()
            .expect("semaphore permit missing. Did you forget to call `poll_ready`?");

        let (reply_tx, reply_rx) = oneshot::channel::<WorkerReply<S::Future, S::Error>>();

        self.tx.send((req, reply_tx)).unwrap_or_else(|_| {
            panic!("buffer worker not running. This is a bug in axum and should never happen. Please file an issue")
        });

        ResponseFuture {
            state: State::Channel { reply_rx },
            permit,
        }
    }
}

pin_project! {
    pub(crate) struct ResponseFuture<F, E> {
        #[pin]
        state: State<F, E>,
        permit: OwnedSemaphorePermit,
    }
}

pin_project! {
    #[project = StateProj]
    enum State<F, E> {
        Channel { reply_rx: oneshot::Receiver<WorkerReply<F, E>> },
        Future { #[pin] future: F },
    }
}

impl<F, E, T> Future for ResponseFuture<F, E>
where
    F: Future<Output = Result<T, E>>,
{
    type Output = Result<T, E>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();

            let new_state = match this.state.as_mut().project() {
                StateProj::Channel { reply_rx } => {
                    let msg = ready!(Pin::new(reply_rx).poll(cx))
                        .expect("buffer worker not running. This is a bug in axum and should never happen. Please file an issue");

                    match msg {
                        WorkerReply::Future(future) => State::Future { future },
                        WorkerReply::Error(err) => return Poll::Ready(Err(err)),
                    }
                }
                StateProj::Future { future } => {
                    return future.poll(cx);
                }
            };

            this.state.set(new_state);
        }
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_buffer() {
        let mut svc = MpscBuffer::new(tower::service_fn(handle));

        let res = svc.ready().await.unwrap().call(42).await.unwrap();

        assert_eq!(res, "foo");
    }

    async fn handle(req: i32) -> Result<&'static str, std::convert::Infallible> {
        assert_eq!(req, 42);
        Ok("foo")
    }
}
