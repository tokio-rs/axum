use super::Response;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use std::{convert::TryInto, fmt};

/// Trait for generating responses from individual parts.
///
/// # Implementing `IntoResponseParts`
///
/// You generally shouldn't have to implement `IntoResponseParts` manually, as axum
/// provides implementations for many common types.
///
/// However it might be necessary if you have a custom error type that you want
/// to return from handlers:
///
/// ```rust
/// use axum::{
///     Router,
///     body::{self, Bytes},
///     routing::get,
///     http::StatusCode,
///     response::{IntoResponseParts, ResponseParts},
/// };
///
/// enum MyError {
///     SomethingWentWrong,
///     SomethingElseWentWrong,
/// }
///
/// impl IntoResponseParts for MyError {
///     fn into_response_parts(self, res: &mut ResponseParts) {
///         let body = match self {
///             MyError::SomethingWentWrong => {
///                 body::boxed(body::Full::from("something went wrong"))
///             },
///             MyError::SomethingElseWentWrong => {
///                 body::boxed(body::Full::from("something else went wrong"))
///             },
///         };
///
///         (StatusCode::INTERNAL_SERVER_ERROR, body).into_response_parts(res)
///     }
/// }
///
/// // `Result<impl IntoResponse, MyError>` can now be returned from handlers
/// let app = Router::new().route("/", get(handler));
///
/// async fn handler() -> Result<(), MyError> {
///     Err(MyError::SomethingWentWrong)
/// }
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Or if you have a custom body type you'll also need to implement
/// `IntoResponseParts` for it:
///
/// ```rust
/// use axum::{
///     body,
///     routing::get,
///     response::{IntoResponseParts, ResponseParts},
///     Router,
/// };
/// use http_body::Body;
/// use http::HeaderMap;
/// use bytes::Bytes;
/// use std::{
///     convert::Infallible,
///     task::{Poll, Context},
///     pin::Pin,
/// };
///
/// struct MyBody;
///
/// // First implement `Body` for `MyBody`. This could for example use
/// // some custom streaming protocol.
/// impl Body for MyBody {
///     type Data = Bytes;
///     type Error = Infallible;
///
///     fn poll_data(
///         self: Pin<&mut Self>,
///         cx: &mut Context<'_>
///     ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
///         # unimplemented!()
///         // ...
///     }
///
///     fn poll_trailers(
///         self: Pin<&mut Self>,
///         cx: &mut Context<'_>
///     ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
///         # unimplemented!()
///         // ...
///     }
/// }
///
/// // Now we can implement `IntoResponseParts` directly for `MyBody`
/// impl IntoResponseParts for MyBody {
///     fn into_response_parts(self, res: &mut ResponseParts) {
///         res.set_body(self)
///     }
/// }
///
/// // `MyBody` can now be returned from handlers.
/// let app = Router::new().route("/", get(|| async { MyBody }));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub trait IntoResponseParts {
    /// Set parts of the response
    fn into_response_parts(self, res: &mut ResponseParts);
}

/// Parts of a response.
///
/// Used with [`IntoResponseParts`].
#[derive(Debug)]
pub struct ResponseParts {
    pub(crate) res: Result<Response, String>,
}

impl ResponseParts {
    /// Insert a header into the response.
    ///
    /// If the header already exists it will be overwritten.
    pub fn insert_header<K, V>(&mut self, key: K, value: V)
    where
        K: TryInto<HeaderName>,
        K::Error: fmt::Display,
        V: TryInto<HeaderValue>,
        V::Error: fmt::Display,
    {
        self.update_headers(key, value, |headers, key, value| {
            headers.insert(key, value);
        });
    }

    /// Append a header to the response.
    ///
    /// If the header already exists it will be appended to.
    pub fn append_header<K, V>(&mut self, key: K, value: V)
    where
        K: TryInto<HeaderName>,
        K::Error: fmt::Display,
        V: TryInto<HeaderValue>,
        V::Error: fmt::Display,
    {
        self.update_headers(key, value, |headers, key, value| {
            headers.append(key, value);
        });
    }

    fn update_headers<K, V, F>(&mut self, key: K, value: V, f: F)
    where
        K: TryInto<HeaderName>,
        K::Error: fmt::Display,
        V: TryInto<HeaderValue>,
        V::Error: fmt::Display,
        F: FnOnce(&mut HeaderMap, HeaderName, HeaderValue),
    {
        if let Ok(response) = &mut self.res {
            let key = match key.try_into() {
                Ok(key) => key,
                Err(err) => {
                    self.res = Err(err.to_string());
                    return;
                }
            };

            let value = match value.try_into() {
                Ok(value) => value,
                Err(err) => {
                    self.res = Err(err.to_string());
                    return;
                }
            };

            f(response.headers_mut(), key, value);
        }
    }

    /// Insert an extension into the response.
    ///
    /// If the extension already exists it will be overwritten.
    pub fn insert_extension<T>(&mut self, extension: T)
    where
        T: Send + Sync + 'static,
    {
        if let Ok(res) = &mut self.res {
            res.extensions_mut().insert(extension);
        }
    }
}

impl Extend<(Option<HeaderName>, HeaderValue)> for ResponseParts {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (Option<HeaderName>, HeaderValue)>,
    {
        if let Ok(res) = &mut self.res {
            res.headers_mut().extend(iter);
        }
    }
}

impl IntoResponseParts for HeaderMap {
    fn into_response_parts(self, res: &mut ResponseParts) {
        res.extend(self);
    }
}

impl<K, V, const N: usize> IntoResponseParts for [(K, V); N]
where
    K: TryInto<HeaderName>,
    K::Error: fmt::Display,
    V: TryInto<HeaderValue>,
    V::Error: fmt::Display,
{
    fn into_response_parts(self, res: &mut ResponseParts) {
        for (key, value) in self {
            res.insert_header(key, value);
        }
    }
}
