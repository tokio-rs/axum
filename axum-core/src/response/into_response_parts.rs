use super::{IntoResponse, Response};
use http::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Extensions, StatusCode,
};
use std::{
    convert::{Infallible, TryInto},
    fmt,
};

/// Trait for adding headers and extensions to a response.
pub trait IntoResponseParts {
    /// The type returned in the event of an error.
    ///
    /// This can be used to fallibly convert types into headers or extensions.
    type Error: IntoResponse;

    /// Set parts of the response
    fn into_response_parts(self, res: ResponseParts) -> Result<ResponseParts, Self::Error>;
}

/// Parts of a response.
///
/// Used with [`IntoResponseParts`].
#[derive(Debug)]
pub struct ResponseParts {
    pub(crate) res: Response,
}

impl ResponseParts {
    /// Gets a reference to the response headers.
    pub fn headers(&self) -> &HeaderMap {
        self.res.headers()
    }

    /// Gets a mutable reference to the response headers.
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        self.res.headers_mut()
    }

    /// Gets a reference to the response extensions.
    pub fn extensions(&self) -> &Extensions {
        self.res.extensions()
    }

    /// Gets a mutable reference to the response extensions.
    pub fn extensions_mut(&mut self) -> &mut Extensions {
        self.res.extensions_mut()
    }
}

impl IntoResponseParts for HeaderMap {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        res.headers_mut().extend(self);
        Ok(res)
    }
}

impl<K, V, const N: usize> IntoResponseParts for [(K, V); N]
where
    K: TryInto<HeaderName>,
    K::Error: fmt::Display,
    V: TryInto<HeaderValue>,
    V::Error: fmt::Display,
{
    type Error = TryIntoHeaderError<K::Error, V::Error>;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        for (key, value) in self {
            let key = key.try_into().map_err(TryIntoHeaderError::key)?;
            let value = value.try_into().map_err(TryIntoHeaderError::value)?;
            res.headers_mut().insert(key, value);
        }

        Ok(res)
    }
}

/// Error returned if converting a value to a header fails.
#[derive(Debug)]
pub struct TryIntoHeaderError<K, V> {
    kind: TryIntoHeaderErrorKind<K, V>,
}

impl<K, V> TryIntoHeaderError<K, V> {
    fn key(err: K) -> Self {
        Self {
            kind: TryIntoHeaderErrorKind::Key(err),
        }
    }

    fn value(err: V) -> Self {
        Self {
            kind: TryIntoHeaderErrorKind::Value(err),
        }
    }
}

#[derive(Debug)]
enum TryIntoHeaderErrorKind<K, V> {
    Key(K),
    Value(V),
}

impl<K, V> IntoResponse for TryIntoHeaderError<K, V>
where
    K: fmt::Display,
    V: fmt::Display,
{
    fn into_response(self) -> Response {
        match self.kind {
            TryIntoHeaderErrorKind::Key(inner) => {
                (StatusCode::INTERNAL_SERVER_ERROR, inner.to_string()).into_response()
            }
            TryIntoHeaderErrorKind::Value(inner) => {
                (StatusCode::INTERNAL_SERVER_ERROR, inner.to_string()).into_response()
            }
        }
    }
}

impl<K, V> fmt::Display for TryIntoHeaderError<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            TryIntoHeaderErrorKind::Key(_) => write!(f, "failed to convert key to a header name"),
            TryIntoHeaderErrorKind::Value(_) => {
                write!(f, "failed to convert value to a header value")
            }
        }
    }
}

impl<K, V> std::error::Error for TryIntoHeaderError<K, V>
where
    K: std::error::Error + 'static,
    V: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            TryIntoHeaderErrorKind::Key(inner) => Some(inner),
            TryIntoHeaderErrorKind::Value(inner) => Some(inner),
        }
    }
}

macro_rules! impl_into_response_parts {
    ( $($ty:ident),* $(,)? ) => {
        #[allow(non_snake_case)]
        impl<$($ty,)*> IntoResponseParts for ($($ty,)*)
        where
            $( $ty: IntoResponseParts, )*
        {
            type Error = Response;

            fn into_response_parts(self, res: ResponseParts) -> Result<ResponseParts, Self::Error> {
                let ($($ty,)*) = self;

                $(
                    let res = match $ty.into_response_parts(res) {
                        Ok(res) => res,
                        Err(err) => {
                            return Err(err.into_response());
                        }
                    };
                )*

                Ok(res)
            }
        }
    }
}

all_the_tuples!(impl_into_response_parts);
