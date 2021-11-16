use bitflags::bitflags;

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
