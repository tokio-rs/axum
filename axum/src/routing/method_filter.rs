use std::{fmt, fmt::{Debug, Formatter}};
use bitflags::bitflags;
use http::Method;

bitflags! {
    /// A filter that matches one or more HTTP methods.
    pub struct MethodFilter: u16 {
        /// Match `DELETE` requests.
        const DELETE =  0b000000010;
        /// Match `GET` requests.
        const GET =     0b000000100;
        /// Match `HEAD` requests.
        const HEAD =    0b000001000;
        /// Match `OPTIONS` requests.
        const OPTIONS = 0b000010000;
        /// Match `PATCH` requests.
        const PATCH =   0b000100000;
        /// Match `POST` requests.
        const POST =    0b001000000;
        /// Match `PUT` requests.
        const PUT =     0b010000000;
        /// Match `TRACE` requests.
        const TRACE =   0b100000000;
    }
}

/// Error type used when converting a [`http::Method`] to a [`MethodFilter`] fails,
/// because there is no matching [`MethodFilter`] for that [`http::Method`].
#[derive(Debug)]
pub struct NoMatchingMethodFilter {
    method: http::Method,
}

impl NoMatchingMethodFilter {
    /// [`NoMatchingMethodFilter`] exposes [`NoMatchingMethodFilter::method()`] to make the [`http::Method`] that was responsible for the error accessible.
    pub fn method(&self) -> &http::Method {
        &self.method
    }
}

impl fmt::Display for NoMatchingMethodFilter {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to match http method: {}", self.method.as_str())
    }
}

impl std::error::Error for NoMatchingMethodFilter {}

impl TryFrom<Method> for MethodFilter {
    type Error = NoMatchingMethodFilter;

    fn try_from(m: Method) -> Result<Self, NoMatchingMethodFilter> {
        match m {
            Method::DELETE => Ok(MethodFilter::DELETE),
            Method::GET => Ok(MethodFilter::GET),
            Method::HEAD => Ok(MethodFilter::HEAD),
            Method::OPTIONS => Ok(MethodFilter::OPTIONS),
            Method::PATCH => Ok(MethodFilter::PATCH),
            Method::POST => Ok(MethodFilter::POST),
            Method::PUT => Ok(MethodFilter::PUT),
            Method::TRACE => Ok(MethodFilter::TRACE),
            other => Err(NoMatchingMethodFilter { method: other}),
        }
    }
}