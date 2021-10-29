# Error handling

In the context of axum an "error" specifically means if a [`Service`]'s
response future resolves to `Err(Service::Error)`. That means async handler
functions can _never_ fail since they always produce a response and their
`Service::Error` type is [`Infallible`]. Returning statuses like 404 or 500
are _not_ errors.

axum works this way because hyper will close the connection, without sending
a response, if an error is encountered. This is not desireable so axum makes
it impossible to forget to handle errors.

Sometimes you need to route to fallible services or apply fallible
middleware in which case you need to handle the errors. That can be done
using things from [`error_handling`].

You can find examples here:
- [Routing to fallible services](#routing-to-fallible-services)
- [Applying fallible middleware](#applying-multiple-middleware)
