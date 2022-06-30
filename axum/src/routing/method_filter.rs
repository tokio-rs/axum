use bitflags::bitflags;
use http::Method;
use axum_core::Error;

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

impl TryFrom<Method> for MethodFilter {
    type Error = Error;

    fn try_from(m: Method) -> Result<Self, Error> {
        match m {
            Method::DELETE => Ok(MethodFilter::DELETE),
            Method::GET => Ok(MethodFilter::GET),
            Method::HEAD => Ok(MethodFilter::HEAD),
            Method::OPTIONS => Ok(MethodFilter::OPTIONS),
            Method::PATCH => Ok(MethodFilter::PATCH),
            Method::POST => Ok(MethodFilter::POST),
            Method::PUT => Ok(MethodFilter::PUT),
            Method::TRACE => Ok(MethodFilter::TRACE),
            _ => Err(Error::new("could not map method")),
        }
    }
}
