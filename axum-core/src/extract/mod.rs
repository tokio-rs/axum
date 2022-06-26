//! Types and traits for extracting data from requests.
//!
//! See [`axum::extract`] for more details.
//!
//! [`axum::extract`]: https://docs.rs/axum/latest/axum/extract/index.html

use self::rejection::*;
use crate::response::IntoResponse;
use async_trait::async_trait;
use http::{
    header::{HeaderMap, HeaderValue, IntoHeaderName},
    Extensions, Method, Request, Uri, Version,
};
use std::{convert::Infallible, marker::PhantomData};

pub mod rejection;

mod request_parts;
mod tuple;

/// Maker type used to signify that an extractor can be run multiple times.
///
/// See [`FromRequest`] for more details.
// TODO(david): naming
#[derive(Debug, Clone, Copy)]
pub struct Mut(Infallible);

/// Maker type used to signify that an extractor can only be run once.
///
/// See [`FromRequest`] for more details.
#[derive(Debug, Clone, Copy)]
pub struct Once(Infallible);

/// Types that can be created from requests.
///
/// See [`axum::extract`] for more details.
///
/// # What is the `R` type parameter?
///
/// `FromRequest`'s `R` type parameter is used to control which parts of the request the extrator
/// has access to.
///
/// `R` is always one of two values:
///
/// - [`Mut`]: This means the extractor has mutable (but limited) access the request. It can access
/// the method, uri, version, headers, and extensions, but it cannot consume the body.
/// - [`Once`]: This includes the same access as [`Mut`] but can als consume the body. This means
/// extractors that use `RequestParts<Once, _>` can only be run once and must run as the last
/// extractor.
///
/// axum uses these types to guarantee that you don't accidentally attempt to extract the request
/// body twice or otherwise run extractors in the wrong order.
///
/// // TODO(david): compile_fail examples
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
/// impl<R, B> FromRequest<R, B> for MyExtractor
/// where
///     B: Send, // required by `async_trait`
/// {
///     type Rejection = http::StatusCode;
///
///     async fn from_request(req: &mut RequestParts<R, B>) -> Result<Self, Self::Rejection> {
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
pub trait FromRequest<R, B>: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    async fn from_request(req: &mut RequestParts<R, B>) -> Result<Self, Self::Rejection>;
}

/// The type used with [`FromRequest`] to extract data from requests.
///
/// Has several convenience methods for getting owned parts of the request.
#[derive(Debug)]
pub struct RequestParts<R, B> {
    method: Method,
    uri: Uri,
    version: Version,
    headers: HeaderMap,
    extensions: Extensions,
    body: Option<B>,
    _marker: PhantomData<fn() -> R>,
}

impl<R, B> RequestParts<R, B> {
    /// Create a new `RequestParts`.
    ///
    /// You generally shouldn't need to construct this type yourself, unless
    /// using extractors outside of axum for example to implement a
    /// [`tower::Service`].
    ///
    /// [`tower::Service`]: https://docs.rs/tower/lastest/tower/trait.Service.html
    pub fn new(req: Request<B>) -> Self {
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
    /// impl<B: Send> FromRequest<B> for MyExtractor {
    ///     type Rejection = Infallible;
    ///
    ///     async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Infallible> {
    ///         let method = req.extract::<Method>().await?;
    ///         let path = req.extract::<Uri>().await?.path().to_owned();
    ///
    ///         todo!()
    ///     }
    /// }
    /// ```
    pub async fn extract<E>(&mut self) -> Result<E, E::Rejection>
    where
        E: FromRequest<R, B>,
    {
        E::from_request(self).await
    }

    // NOTE: `method_mut`, `version_mut`, and `uri_mut` should be fine to call for `RequestParts<Mut, _>`
    // because they don't remove any data that could make follow up extractors fail.
    //
    // Whereas `extensions_mut` does since you can make `Extension` fail by removing all extensions
    // with `std::mem::take`

    /// Gets a reference the request method.
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// Gets a mutable reference to the request method.
    pub fn method_mut(&mut self) -> &mut Method {
        &mut self.method
    }

    /// Gets a reference the request URI.
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

    /// Inserts a header, overriding any previous value.
    ///
    /// The previous value is returned, if any.
    pub fn insert_header<K>(&mut self, key: K, value: HeaderValue) -> Option<HeaderValue>
    where
        K: IntoHeaderName,
    {
        self.headers.insert(key, value)
    }

    /// Appends a header, without overriding any previous value.
    ///
    /// If the map did not previously have this key present, then false is returned.
    pub fn append_header<K>(&mut self, key: K, value: HeaderValue) -> bool
    where
        K: IntoHeaderName,
    {
        self.headers.append(key, value)
    }

    /// Gets a reference to the request extensions.
    pub fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    /// Insert a new request extension.
    pub fn insert_extension<T>(&mut self, extension: T)
    where
        T: Send + Sync + 'static,
    {
        self.extensions.insert(extension);
    }

    /// Remove a type from the request extensions.
    pub fn remove_extension<T>(&mut self) -> Option<T>
    where
        T: Send + Sync + 'static,
    {
        self.extensions.remove::<T>()
    }

    /// Gets a reference to the request body.
    ///
    /// Returns `None` if the body has been taken by another extractor.
    pub fn body(&self) -> Option<&B> {
        self.body.as_ref()
    }
}

impl<B> RequestParts<Mut, B> {
    /// Convert this `RequestParts` back into a [`Request`], infallibly.
    ///
    /// This can never fail (as opposed to `RequestParts::try_into_request`) because
    /// you cannot remove the body from a `RequestParts<Mut, _>`.
    pub fn into_request(self) -> Request<B> {
        let Self {
            method,
            uri,
            version,
            headers,
            extensions,
            mut body,
            _marker: _,
        } = self;

        let mut req = if let Some(body) = body.take() {
            Request::new(body)
        } else {
            // you cannot remove the body from a `RequestParts<Mut, _>` so it'll never be `None`
            unreachable!();
        };

        *req.method_mut() = method;
        *req.uri_mut() = uri;
        *req.version_mut() = version;
        *req.headers_mut() = headers;
        *req.extensions_mut() = extensions;

        req
    }
}

impl<B> RequestParts<Once, B> {
    /// Convert this `RequestParts` back into a [`Request`].
    ///
    /// Fails if The request body has been extracted, that is [`take_body`] has
    /// been called.
    ///
    /// [`take_body`]: RequestParts::take_body
    pub fn try_into_request(self) -> Result<Request<B>, BodyAlreadyExtracted> {
        let Self {
            method,
            uri,
            version,
            headers,
            extensions,
            mut body,
            _marker: _,
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

    #[allow(clippy::wrong_self_convention)]
    pub(crate) fn to_mut(&mut self) -> RequestParts<Mut, B> {
        RequestParts {
            method: self.method().clone(),
            uri: self.uri().clone(),
            version: self.version(),
            headers: std::mem::take(self.headers_mut()),
            extensions: std::mem::take(self.extensions_mut()),
            body: Some(
                self.take_body()
                    .expect("`to_mut` must be called before any extractors"),
            ),
            _marker: PhantomData,
        }
    }

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
impl<R, B> FromRequest<R, B> for ()
where
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(_: &mut RequestParts<R, B>) -> Result<Self, Self::Rejection> {
        Ok(())
    }
}

#[async_trait]
impl<T, B, R> FromRequest<R, B> for Option<T>
where
    T: FromRequest<R, B>,
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<R, B>) -> Result<Option<T>, Self::Rejection> {
        Ok(T::from_request(req).await.ok())
    }
}

#[async_trait]
impl<T, B, R> FromRequest<R, B> for Result<T, T::Rejection>
where
    T: FromRequest<R, B>,
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<R, B>) -> Result<Self, Self::Rejection> {
        Ok(T::from_request(req).await)
    }
}
