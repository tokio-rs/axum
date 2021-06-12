//! HTTP body utilities.

use bytes::Bytes;
use http_body::{Empty, Full, SizeHint};
use pin_project::pin_project;
use std::{
    error::Error as StdError,
    fmt,
    pin::Pin,
    task::{Context, Poll},
};
use tower::BoxError;

pub use hyper::body::Body;

/// A boxed [`Body`] trait object.
///
/// This is used in tower-web as the response body type for applications. Its necessary to unify
/// multiple response bodies types into one.
pub struct BoxBody {
    // when we've gotten rid of `BoxStdError` we should be able to change the error type to
    // `BoxError`
    inner: Pin<Box<dyn http_body::Body<Data = Bytes, Error = BoxStdError> + Send + Sync + 'static>>,
}

impl BoxBody {
    /// Create a new `BoxBody`.
    pub fn new<B>(body: B) -> Self
    where
        B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        B::Error: Into<BoxError> + Send + Sync + 'static,
    {
        Self {
            inner: Box::pin(body.map_err(|error| BoxStdError(error.into()))),
        }
    }
}

impl fmt::Debug for BoxBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxBody").finish()
    }
}

impl http_body::Body for BoxBody {
    type Data = Bytes;
    type Error = BoxStdError;

    fn poll_data(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        self.inner.as_mut().poll_data(cx)
    }

    fn poll_trailers(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        self.inner.as_mut().poll_trailers(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

impl<B> From<B> for BoxBody
where
    B: Into<Bytes>,
{
    fn from(s: B) -> Self {
        BoxBody::new(Full::from(s.into()))
    }
}

/// A boxed error trait object that implements [`std::error::Error`].
///
/// This is necessary for compatibility with middleware that changes the error
/// type of the response body.
#[derive(Debug)]
pub struct BoxStdError(pub(crate) tower::BoxError);

impl StdError for BoxStdError {
    fn source(&self) -> std::option::Option<&(dyn StdError + 'static)> {
        self.0.source()
    }
}

impl fmt::Display for BoxStdError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Type that combines two body types into one.
#[pin_project]
#[derive(Debug)]
pub struct Or<A, B>(#[pin] Either<A, B>);

impl<A, B> Or<A, B> {
    #[inline]
    pub(crate) fn a(a: A) -> Self {
        Or(Either::A(a))
    }

    #[inline]
    pub(crate) fn b(b: B) -> Self {
        Or(Either::B(b))
    }
}

impl<A, B> Default for Or<A, B> {
    fn default() -> Self {
        Self(Either::Empty(Empty::new()))
    }
}

#[pin_project(project = EitherProj)]
#[derive(Debug)]
enum Either<A, B> {
    Empty(Empty<Bytes>), // required for `Default`
    A(#[pin] A),
    B(#[pin] B),
}

impl<A, B> http_body::Body for Or<A, B>
where
    A: http_body::Body<Data = Bytes>,
    A::Error: Into<BoxError>,
    B: http_body::Body<Data = Bytes>,
    B::Error: Into<BoxError>,
{
    type Data = Bytes;
    type Error = BoxStdError;

    #[inline]
    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        match self.project().0.project() {
            EitherProj::Empty(inner) => Pin::new(inner).poll_data(cx).map(map_option_error),
            EitherProj::A(inner) => inner.poll_data(cx).map(map_option_error),
            EitherProj::B(inner) => inner.poll_data(cx).map(map_option_error),
        }
    }

    #[inline]
    fn poll_trailers(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        match self.project().0.project() {
            EitherProj::Empty(inner) => Pin::new(inner)
                .poll_trailers(cx)
                .map_err(Into::into)
                .map_err(BoxStdError),
            EitherProj::A(inner) => inner
                .poll_trailers(cx)
                .map_err(Into::into)
                .map_err(BoxStdError),
            EitherProj::B(inner) => inner
                .poll_trailers(cx)
                .map_err(Into::into)
                .map_err(BoxStdError),
        }
    }

    #[inline]
    fn size_hint(&self) -> SizeHint {
        match &self.0 {
            Either::Empty(inner) => inner.size_hint(),
            Either::A(inner) => inner.size_hint(),
            Either::B(inner) => inner.size_hint(),
        }
    }

    #[inline]
    fn is_end_stream(&self) -> bool {
        match &self.0 {
            Either::Empty(inner) => inner.is_end_stream(),
            Either::A(inner) => inner.is_end_stream(),
            Either::B(inner) => inner.is_end_stream(),
        }
    }
}

fn map_option_error<T, E>(opt: Option<Result<T, E>>) -> Option<Result<T, BoxStdError>>
where
    E: Into<BoxError>,
{
    opt.map(|result| result.map_err(Into::<BoxError>::into).map_err(BoxStdError))
}
