use super::Response;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use std::{convert::TryInto, fmt};

/// Trait for adding headers and extensions to a response.
///
/// You generally don't need to implement this trait manually. It's recommended instead to rely
/// on the implementations in axum.
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
    /// If the header already exists, it will be overwritten.
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
