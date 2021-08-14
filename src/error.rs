use std::{error::Error as StdError, fmt};
use tower::BoxError;

/// Errors that can happen when using axum.
#[derive(Debug)]
pub struct Error {
    inner: BoxError,
}

impl Error {
    pub(crate) fn new(error: impl Into<BoxError>) -> Self {
        Self {
            inner: error.into(),
        }
    }

    pub(crate) fn downcast<T>(self) -> Result<T, Self>
    where
        T: StdError + 'static,
    {
        match self.inner.downcast::<T>() {
            Ok(t) => Ok(*t),
            Err(err) => Err(*err.downcast().unwrap()),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&*self.inner)
    }
}
