In axum a "handler" is an async function that accepts zero or more
["extractors"](#extractors) as arguments and returns something that
can be converted [into a response](crate::response).

Handlers is where your application logic lives and axum applications are built
by routing between handlers.

[`debug_handler`]: https://docs.rs/axum-debug/latest/axum_debug/attr.debug_handler.html
