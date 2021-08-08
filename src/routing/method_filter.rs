use bitflags::bitflags;
use http::Method;

bitflags! {
    /// A filter that matches one or more HTTP methods.
    pub struct MethodFilter: u16 {
        /// Match `CONNECT` requests.
        const CONNECT = 0b000000001;
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
        /// Match `POSt` requests.
        const POST =    0b001000000;
        /// Match `PUT` requests.
        const PUT =     0b010000000;
        /// Match `TRACE` requests.
        const TRACE =   0b100000000;
    }
}

impl MethodFilter {
    #[allow(clippy::match_like_matches_macro)]
    pub(crate) fn matches(self, method: &Method) -> bool {
        let method = match *method {
            Method::CONNECT => Self::CONNECT,
            Method::DELETE => Self::DELETE,
            Method::GET => Self::GET,
            Method::HEAD => Self::HEAD,
            Method::OPTIONS => Self::OPTIONS,
            Method::PATCH => Self::PATCH,
            Method::POST => Self::POST,
            Method::PUT => Self::PUT,
            Method::TRACE => Self::TRACE,
            _ => return false,
        };
        self.contains(method)
    }
}
