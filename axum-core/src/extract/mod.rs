//! Types and traits for extracting data from requests.
//!
//! See [`axum::extract`] for more details.
//!
//! [`axum::extract`]: https://docs.rs/axum/latest/axum/extract/index.html

use self::rejection::*;
use crate::response::IntoResponse;
use async_trait::async_trait;
use http::{
    header::IntoHeaderName, Extensions, HeaderMap, HeaderValue, Method, Request, Uri, Version,
};
use std::{convert::Infallible, marker::PhantomData, sync::Arc};

pub mod rejection;

mod from_ref;
mod request_parts;
mod tuple;

pub use self::from_ref::FromRef;

/// Marker type used to signal that a `FromRequest` implementation can run multiple times.
///
/// See [`FromRequest`] for more details.
#[derive(Debug, Clone, Copy)]
pub enum Mut {}

/// Marker type used to signal that a `FromRequest` implementation can only run once.
///
/// Extractors that implement `FromRequest<Once, _, _>` must be the last argument to handler
/// functions.
///
/// See [`FromRequest`] for more details.
#[derive(Debug, Clone, Copy)]
pub enum Once {}

/// Types that can be created from requests.
///
/// See [`axum::extract`] for more details.
///
/// # What is the `B` type parameter?
///
/// `FromRequest` is generic over the request body (the `B` in
/// [`http::Request<B>`]). This is to allow `FromRequest` to be usable with any
/// type of request body. This is necessary because some middleware change the
/// request body, for example to add timeouts.
///
/// If you're writing your own `FromRequest` that wont be used outside your
/// application, and not using any middleware that changes the request body, you
/// can most likely use `axum::body::Body`.
///
/// If you're writing a library that's intended for others to use, it's recommended
/// to keep the generic type parameter:
///
/// ```rust
/// use axum::{
///     async_trait,
///     extract::{FromRequest, RequestParts},
/// };
///
/// struct MyExtractor;
///
/// #[async_trait]
/// impl<S, B> FromRequest<S, B> for MyExtractor
/// where
///     // these bounds are required by `async_trait`
///     B: Send,
///     S: Send + Sync,
/// {
///     type Rejection = http::StatusCode;
///
///     async fn from_request(req: &mut RequestParts<S, B>) -> Result<Self, Self::Rejection> {
///         // ...
///         # unimplemented!()
///     }
/// }
/// ```
///
/// This ensures your extractor is as flexible as possible.
///
/// [`http::Request<B>`]: http::Request
/// [`axum::extract`]: https://docs.rs/axum/latest/axum/extract/index.html
#[async_trait]
pub trait FromRequest<M, S, B>: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    async fn from_request(req: &mut RequestParts<M, S, B>) -> Result<Self, Self::Rejection>;
}

/// The type used with [`FromRequest`] to extract data from requests.
///
/// Has several convenience methods for getting owned parts of the request.
#[derive(Debug)]
pub struct RequestParts<M, S, B> {
    pub(crate) state: Arc<S>,
    method: Method,
    uri: Uri,
    version: Version,
    headers: HeaderMap,
    extensions: Extensions,
    body: Option<B>,
    _marker: PhantomData<fn() -> M>,
}

impl<M, B> RequestParts<M, (), B> {
    /// Create a new `RequestParts` without any state.
    ///
    /// You generally shouldn't need to construct this type yourself, unless
    /// using extractors outside of axum for example to implement a
    /// [`tower::Service`].
    ///
    /// [`tower::Service`]: https://docs.rs/tower/lastest/tower/trait.Service.html
    pub fn new(req: Request<B>) -> Self {
        Self::with_state((), req)
    }
}

impl<M, S, B> RequestParts<M, S, B> {
    /// Create a new `RequestParts` with the given state.
    ///
    /// You generally shouldn't need to construct this type yourself, unless
    /// using extractors outside of axum for example to implement a
    /// [`tower::Service`].
    ///
    /// [`tower::Service`]: https://docs.rs/tower/lastest/tower/trait.Service.html
    pub fn with_state(state: S, req: Request<B>) -> Self {
        Self::with_state_arc(Arc::new(state), req)
    }

    /// Create a new `RequestParts` with the given [`Arc`]'ed state.
    ///
    /// You generally shouldn't need to construct this type yourself, unless
    /// using extractors outside of axum for example to implement a
    /// [`tower::Service`].
    ///
    /// [`tower::Service`]: https://docs.rs/tower/lastest/tower/trait.Service.html
    pub fn with_state_arc(state: Arc<S>, req: Request<B>) -> Self {
        let (
            http::request::Parts {
                method,
                uri,
                version,
                headers,
                extensions,
                ..
            },
            body,
        ) = req.into_parts();

        RequestParts {
            state,
            method,
            uri,
            version,
            headers,
            extensions,
            body: Some(body),
            _marker: PhantomData,
        }
    }

    /// Apply an extractor to this `RequestParts`.
    ///
    /// `req.extract::<Extractor>()` is equivalent to `Extractor::from_request(req)`.
    /// This function simply exists as a convenience.
    ///
    /// # Example
    ///
    /// ```
    /// # struct MyExtractor {}
    ///
    /// use std::convert::Infallible;
    ///
    /// use async_trait::async_trait;
    /// use axum::extract::{FromRequest, RequestParts};
    /// use http::{Method, Uri};
    ///
    /// #[async_trait]
    /// impl<S, B> FromRequest<S, B> for MyExtractor
    /// where
    ///     B: Send,
    ///     S: Send + Sync,
    /// {
    ///     type Rejection = Infallible;
    ///
    ///     async fn from_request(req: &mut RequestParts<S, B>) -> Result<Self, Infallible> {
    ///         let method = req.extract::<Method>().await?;
    ///         let path = req.extract::<Uri>().await?.path().to_owned();
    ///
    ///         todo!()
    ///     }
    /// }
    /// ```
    pub async fn extract<E>(&mut self) -> Result<E, E::Rejection>
    where
        E: FromRequest<M, S, B>,
    {
        E::from_request(self).await
    }

    /// Try and convert this `RequestParts` back into a [`Request`].
    ///
    /// Fails if The request body has been extracted, that is [`take_body`] has
    /// been called.
    ///
    /// [`take_body`]: RequestParts::take_body
    pub fn try_into_request(self) -> Result<Request<B>, BodyAlreadyExtracted> {
        let Self {
            state: _,
            method,
            uri,
            version,
            headers,
            extensions,
            mut body,
            _marker,
        } = self;

        let mut req = if let Some(body) = body.take() {
            Request::new(body)
        } else {
            return Err(BodyAlreadyExtracted);
        };

        *req.method_mut() = method;
        *req.uri_mut() = uri;
        *req.version_mut() = version;
        *req.headers_mut() = headers;
        *req.extensions_mut() = extensions;

        Ok(req)
    }

    /// Gets a reference to the request method.
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// Gets a mutable reference to the request method.
    pub fn method_mut(&mut self) -> &mut Method {
        &mut self.method
    }

    /// Gets a reference to the request URI.
    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    /// Gets a mutable reference to the request URI.
    pub fn uri_mut(&mut self) -> &mut Uri {
        &mut self.uri
    }

    /// Get the request HTTP version.
    pub fn version(&self) -> Version {
        self.version
    }

    /// Gets a mutable reference to the request HTTP version.
    pub fn version_mut(&mut self) -> &mut Version {
        &mut self.version
    }

    /// Gets a reference to the request headers.
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Gets a reference to the request extensions.
    pub fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    /// Gets a reference to the request body.
    ///
    /// Returns `None` if the body has been taken by another extractor.
    pub fn body(&self) -> Option<&B> {
        self.body.as_ref()
    }

    /// Get a reference to the state.
    pub fn state(&self) -> &S {
        &self.state
    }

    /// Inserts a header, overriding any previous value.
    ///
    /// The previous value is returned, if any.
    pub fn insert_header<K>(&mut self, key: K, value: HeaderValue) -> Option<HeaderValue>
    where
        K: IntoHeaderName,
    {
        self.headers.insert(key, value)
    }

    /// Appends a header, without overriding any previous values.
    ///
    /// If the map did not previously have this key present, then false is returned.
    pub fn append_header<K>(&mut self, key: K, value: HeaderValue) -> bool
    where
        K: IntoHeaderName,
    {
        self.headers.append(key, value)
    }

    /// Insert a new request extension.
    pub fn insert_extension<T>(&mut self, extension: T)
    where
        T: Send + Sync + 'static,
    {
        self.extensions.insert(extension);
    }
}

impl<S, B> RequestParts<Mut, S, B> {
    /// Convert this `RequestParts` back into a [`Request`].
    pub fn into_request(self) -> Request<B> {
        self.try_into_request()
            .expect("body removed from `RequestParts<Mut, _, _>`. This should never happen")
    }
}

impl<S, B> RequestParts<Once, S, B> {
    /// Gets a mutable reference to the request headers.
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        &mut self.headers
    }

    /// Gets a mutable reference to the request extensions.
    pub fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.extensions
    }

    /// Gets a mutable reference to the request body.
    ///
    /// Returns `None` if the body has been taken by another extractor.
    // this returns `&mut Option<B>` rather than `Option<&mut B>` such that users can use it to set the body.
    pub fn body_mut(&mut self) -> &mut Option<B> {
        &mut self.body
    }

    /// Takes the body out of the request, leaving a `None` in its place.
    pub fn take_body(&mut self) -> Option<B> {
        self.body.take()
    }
}

#[async_trait]
impl<M, S, T, B> FromRequest<M, S, B> for Option<T>
where
    T: FromRequest<M, S, B>,
    B: Send,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<M, S, B>) -> Result<Option<T>, Self::Rejection> {
        Ok(T::from_request(req).await.ok())
    }
}

#[async_trait]
impl<M, S, T, B> FromRequest<M, S, B> for Result<T, T::Rejection>
where
    T: FromRequest<M, S, B>,
    B: Send,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<M, S, B>) -> Result<Self, Self::Rejection> {
        Ok(T::from_request(req).await)
    }
}
