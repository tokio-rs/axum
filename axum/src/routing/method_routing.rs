use crate::{
    body::{boxed, Body, Bytes, Empty, HttpBody},
    error_handling::{HandleError, HandleErrorLayer},
    handler::Handler,
    http::{Method, Request, StatusCode},
    response::Response,
    routing::{Fallback, MethodFilter, Route},
    BoxError,
};
use bytes::BytesMut;
use std::{
    convert::Infallible,
    fmt,
    marker::PhantomData,
    task::{Context, Poll},
};
use tower::{service_fn, ServiceBuilder, ServiceExt};
use tower_http::map_response_body::MapResponseBodyLayer;
use tower_layer::Layer;
use tower_service::Service;

macro_rules! top_level_service_fn {
    (
        $name:ident, GET
    ) => {
        top_level_service_fn!(
            /// Route `GET` requests to the given service.
            ///
            /// # Example
            ///
            /// ```rust
            /// use axum::{
            ///     http::Request,
            ///     Router,
            ///     routing::get_service,
            /// };
            /// use http::Response;
            /// use std::convert::Infallible;
            /// use hyper::Body;
            ///
            /// let service = tower::service_fn(|request: Request<Body>| async {
            ///     Ok::<_, Infallible>(Response::new(Body::empty()))
            /// });
            ///
            /// // Requests to `GET /` will go to `service`.
            /// let app = Router::new().route("/", get_service(service));
            /// # async {
            /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
            /// # };
            /// ```
            ///
            /// Note that `get` routes will also be called for `HEAD` requests but will have
            /// the response body removed. Make sure to add explicit `HEAD` routes
            /// afterwards.
            $name,
            GET
        );
    };

    (
        $name:ident, $method:ident
    ) => {
        top_level_service_fn!(
            #[doc = concat!("Route `", stringify!($method) ,"` requests to the given service.")]
            ///
            /// See [`get_service`] for an example.
            $name,
            $method
        );
    };

    (
        $(#[$m:meta])+
        $name:ident, $method:ident
    ) => {
        $(#[$m])+
        pub fn $name<S, ReqBody, ResBody>(svc: S) -> MethodRouter<ReqBody, S::Error>
        where
            S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
            S::Future: Send + 'static,
            ResBody: HttpBody<Data = Bytes> + Send + 'static,
            ResBody::Error: Into<BoxError>,
        {
            on_service(MethodFilter::$method, svc)
        }
    };
}

macro_rules! top_level_handler_fn {
    (
        $name:ident, GET
    ) => {
        top_level_handler_fn!(
            /// Route `GET` requests to the given handler.
            ///
            /// # Example
            ///
            /// ```rust
            /// use axum::{
            ///     routing::get,
            ///     Router,
            /// };
            ///
            /// async fn handler() {}
            ///
            /// // Requests to `GET /` will go to `handler`.
            /// let app = Router::new().route("/", get(handler));
            /// # async {
            /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
            /// # };
            /// ```
            ///
            /// Note that `get` routes will also be called for `HEAD` requests but will have
            /// the response body removed. Make sure to add explicit `HEAD` routes
            /// afterwards.
            $name,
            GET
        );
    };

    (
        $name:ident, $method:ident
    ) => {
        top_level_handler_fn!(
            #[doc = concat!("Route `", stringify!($method) ,"` requests to the given handler.")]
            ///
            /// See [`get`] for an example.
            $name,
            $method
        );
    };

    (
        $(#[$m:meta])+
        $name:ident, $method:ident
    ) => {
        $(#[$m])+
        pub fn $name<H, T, B>(handler: H) -> MethodRouter<B, Infallible>
        where
            H: Handler<T, B>,
            B: Send + 'static,
            T: 'static,
        {
            on(MethodFilter::$method, handler)
        }
    };
}

macro_rules! chained_service_fn {
    (
        $name:ident, GET
    ) => {
        chained_service_fn!(
            /// Chain an additional service that will only accept `GET` requests.
            ///
            /// # Example
            ///
            /// ```rust
            /// use axum::{
            ///     http::Request,
            ///     Router,
            ///     routing::post_service,
            /// };
            /// use http::Response;
            /// use std::convert::Infallible;
            /// use hyper::Body;
            ///
            /// let service = tower::service_fn(|request: Request<Body>| async {
            ///     Ok::<_, Infallible>(Response::new(Body::empty()))
            /// });
            ///
            /// let other_service = tower::service_fn(|request: Request<Body>| async {
            ///     Ok::<_, Infallible>(Response::new(Body::empty()))
            /// });
            ///
            /// // Requests to `GET /` will go to `service` and `POST /` will go to
            /// // `other_service`.
            /// let app = Router::new().route("/", post_service(service).get_service(other_service));
            /// # async {
            /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
            /// # };
            /// ```
            ///
            /// Note that `get` routes will also be called for `HEAD` requests but will have
            /// the response body removed. Make sure to add explicit `HEAD` routes
            /// afterwards.
            $name,
            GET
        );
    };

    (
        $name:ident, $method:ident
    ) => {
        chained_service_fn!(
            #[doc = concat!("Chain an additional service that will only accept `", stringify!($method),"` requests.")]
            ///
            /// See [`MethodRouter::get_service`] for an example.
            $name,
            $method
        );
    };

    (
        $(#[$m:meta])+
        $name:ident, $method:ident
    ) => {
        $(#[$m])+
        pub fn $name<S, ResBody>(self, svc: S) -> Self
        where
            S: Service<Request<ReqBody>, Response = Response<ResBody>, Error = E>
                + Clone
                + Send
                + 'static,
            S::Future: Send + 'static,
            ResBody: HttpBody<Data = Bytes> + Send + 'static,
            ResBody::Error: Into<BoxError>,
        {
            self.on_service(MethodFilter::$method, svc)
        }
    };
}

macro_rules! chained_handler_fn {
    (
        $name:ident, GET
    ) => {
        chained_handler_fn!(
            /// Chain an additional handler that will only accept `GET` requests.
            ///
            /// # Example
            ///
            /// ```rust
            /// use axum::{routing::post, Router};
            ///
            /// async fn handler() {}
            ///
            /// async fn other_handler() {}
            ///
            /// // Requests to `GET /` will go to `handler` and `POST /` will go to
            /// // `other_handler`.
            /// let app = Router::new().route("/", post(handler).get(other_handler));
            /// # async {
            /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
            /// # };
            /// ```
            ///
            /// Note that `get` routes will also be called for `HEAD` requests but will have
            /// the response body removed. Make sure to add explicit `HEAD` routes
            /// afterwards.
            $name,
            GET
        );
    };

    (
        $name:ident, $method:ident
    ) => {
        chained_handler_fn!(
            #[doc = concat!("Chain an additional handler that will only accept `", stringify!($method),"` requests.")]
            ///
            /// See [`MethodRouter::get`] for an example.
            $name,
            $method
        );
    };

    (
        $(#[$m:meta])+
        $name:ident, $method:ident
    ) => {
        $(#[$m])+
        pub fn $name<H, T>(self, handler: H) -> Self
        where
            H: Handler<T, B>,
            T: 'static,
        {
            self.on(MethodFilter::$method, handler)
        }
    };
}

top_level_service_fn!(delete_service, DELETE);
top_level_service_fn!(get_service, GET);
top_level_service_fn!(head_service, HEAD);
top_level_service_fn!(options_service, OPTIONS);
top_level_service_fn!(patch_service, PATCH);
top_level_service_fn!(post_service, POST);
top_level_service_fn!(put_service, PUT);
top_level_service_fn!(trace_service, TRACE);

/// Route requests with the given method to the service.
///
/// # Example
///
/// ```rust
/// use axum::{
///     http::Request,
///     routing::on,
///     Router,
///     routing::{MethodFilter, on_service},
/// };
/// use http::Response;
/// use std::convert::Infallible;
/// use hyper::Body;
///
/// let service = tower::service_fn(|request: Request<Body>| async {
///     Ok::<_, Infallible>(Response::new(Body::empty()))
/// });
///
/// // Requests to `POST /` will go to `service`.
/// let app = Router::new().route("/", on_service(MethodFilter::POST, service));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub fn on_service<S, ReqBody, ResBody>(
    filter: MethodFilter,
    svc: S,
) -> MethodRouter<ReqBody, S::Error>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
{
    MethodRouter::new().on_service(filter, svc)
}

/// Route requests to the given service regardless of its method.
///
/// # Example
///
/// ```rust
/// use axum::{
///     http::Request,
///     Router,
///     routing::any_service,
/// };
/// use http::Response;
/// use std::convert::Infallible;
/// use hyper::Body;
///
/// let service = tower::service_fn(|request: Request<Body>| async {
///     Ok::<_, Infallible>(Response::new(Body::empty()))
/// });
///
/// // All requests to `/` will go to `service`.
/// let app = Router::new().route("/", any_service(service));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Additional methods can still be chained:
///
/// ```rust
/// use axum::{
///     http::Request,
///     Router,
///     routing::any_service,
/// };
/// use http::Response;
/// use std::convert::Infallible;
/// use hyper::Body;
///
/// let service = tower::service_fn(|request: Request<Body>| async {
///     # Ok::<_, Infallible>(Response::new(Body::empty()))
///     // ...
/// });
///
/// let other_service = tower::service_fn(|request: Request<Body>| async {
///     # Ok::<_, Infallible>(Response::new(Body::empty()))
///     // ...
/// });
///
/// // `POST /` goes to `other_service`. All other requests go to `service`
/// let app = Router::new().route("/", any_service(service).post_service(other_service));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub fn any_service<S, ReqBody, ResBody>(svc: S) -> MethodRouter<ReqBody, S::Error>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
{
    MethodRouter::new().fallback(svc).skip_allow_header()
}

top_level_handler_fn!(delete, DELETE);
top_level_handler_fn!(get, GET);
top_level_handler_fn!(head, HEAD);
top_level_handler_fn!(options, OPTIONS);
top_level_handler_fn!(patch, PATCH);
top_level_handler_fn!(post, POST);
top_level_handler_fn!(put, PUT);
top_level_handler_fn!(trace, TRACE);

/// Route requests with the given method to the handler.
///
/// # Example
///
/// ```rust
/// use axum::{
///     routing::on,
///     Router,
///     routing::MethodFilter,
/// };
///
/// async fn handler() {}
///
/// // Requests to `POST /` will go to `handler`.
/// let app = Router::new().route("/", on(MethodFilter::POST, handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub fn on<H, T, B>(filter: MethodFilter, handler: H) -> MethodRouter<B, Infallible>
where
    H: Handler<T, B>,
    B: Send + 'static,
    T: 'static,
{
    MethodRouter::new().on(filter, handler)
}

/// Route requests with the given handler regardless of the method.
///
/// # Example
///
/// ```rust
/// use axum::{
///     routing::any,
///     Router,
/// };
///
/// async fn handler() {}
///
/// // All requests to `/` will go to `handler`.
/// let app = Router::new().route("/", any(handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Additional methods can still be chained:
///
/// ```rust
/// use axum::{
///     routing::any,
///     Router,
/// };
///
/// async fn handler() {}
///
/// async fn other_handler() {}
///
/// // `POST /` goes to `other_handler`. All other requests go to `handler`
/// let app = Router::new().route("/", any(handler).post(other_handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub fn any<H, T, B>(handler: H) -> MethodRouter<B, Infallible>
where
    H: Handler<T, B>,
    B: Send + 'static,
    T: 'static,
{
    MethodRouter::new()
        .fallback_boxed_response_body(handler.into_service())
        .skip_allow_header()
}

/// A [`Service`] that accepts requests based on a [`MethodFilter`] and
/// allows chaining additional handlers and services.
pub struct MethodRouter<B = Body, E = Infallible> {
    get: Option<Route<B, E>>,
    head: Option<Route<B, E>>,
    delete: Option<Route<B, E>>,
    options: Option<Route<B, E>>,
    patch: Option<Route<B, E>>,
    post: Option<Route<B, E>>,
    put: Option<Route<B, E>>,
    trace: Option<Route<B, E>>,
    fallback: Fallback<B, E>,
    allow_header: AllowHeader,
    _request_body: PhantomData<fn() -> (B, E)>,
}

#[derive(Clone)]
enum AllowHeader {
    /// No `Allow` header value has been built-up yet. This is the default state
    None,
    /// Don't set an `Allow` header. This is used when `any` or `any_service` are called.
    Skip,
    /// The current value of the `Allow` header.
    Bytes(BytesMut),
}

impl<B, E> fmt::Debug for MethodRouter<B, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MethodRouter")
            .field("get", &self.get)
            .field("head", &self.head)
            .field("delete", &self.delete)
            .field("options", &self.options)
            .field("patch", &self.patch)
            .field("post", &self.post)
            .field("put", &self.put)
            .field("trace", &self.trace)
            .field("fallback", &self.fallback)
            .finish()
    }
}

impl<B, E> MethodRouter<B, E> {
    /// Create a default `MethodRouter` that will respond with `405 Method Not Allowed` to all
    /// requests.
    pub fn new() -> Self {
        let fallback = Route::new(service_fn(|_: Request<B>| async {
            let mut response = Response::new(boxed(Empty::new()));
            *response.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
            Ok(response)
        }));

        Self {
            get: None,
            head: None,
            delete: None,
            options: None,
            patch: None,
            post: None,
            put: None,
            trace: None,
            allow_header: AllowHeader::None,
            fallback: Fallback::Default(fallback),
            _request_body: PhantomData,
        }
    }
}

impl<B> MethodRouter<B, Infallible>
where
    B: Send + 'static,
{
    /// Chain an additional handler that will accept requests matching the given
    /// `MethodFilter`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::{
    ///     routing::get,
    ///     Router,
    ///     routing::MethodFilter
    /// };
    ///
    /// async fn handler() {}
    ///
    /// async fn other_handler() {}
    ///
    /// // Requests to `GET /` will go to `handler` and `DELETE /` will go to
    /// // `other_handler`
    /// let app = Router::new().route("/", get(handler).on(MethodFilter::DELETE, other_handler));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn on<H, T>(self, filter: MethodFilter, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: 'static,
    {
        self.on_service_boxed_response_body(filter, handler.into_service())
    }

    chained_handler_fn!(delete, DELETE);
    chained_handler_fn!(get, GET);
    chained_handler_fn!(head, HEAD);
    chained_handler_fn!(options, OPTIONS);
    chained_handler_fn!(patch, PATCH);
    chained_handler_fn!(post, POST);
    chained_handler_fn!(put, PUT);
    chained_handler_fn!(trace, TRACE);
}

impl<ReqBody, E> MethodRouter<ReqBody, E> {
    /// Chain an additional service that will accept requests matching the given
    /// `MethodFilter`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::{
    ///     http::Request,
    ///     Router,
    ///     routing::{MethodFilter, on_service},
    /// };
    /// use http::Response;
    /// use std::convert::Infallible;
    /// use hyper::Body;
    ///
    /// let service = tower::service_fn(|request: Request<Body>| async {
    ///     Ok::<_, Infallible>(Response::new(Body::empty()))
    /// });
    ///
    /// // Requests to `DELETE /` will go to `service`
    /// let app = Router::new().route("/", on_service(MethodFilter::DELETE, service));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn on_service<S, ResBody>(self, filter: MethodFilter, svc: S) -> Self
    where
        S: Service<Request<ReqBody>, Response = Response<ResBody>, Error = E>
            + Clone
            + Send
            + 'static,
        S::Future: Send + 'static,
        ResBody: HttpBody<Data = Bytes> + Send + 'static,
        ResBody::Error: Into<BoxError>,
    {
        self.on_service_boxed_response_body(filter, svc.map_response(|res| res.map(boxed)))
    }

    chained_service_fn!(delete_service, DELETE);
    chained_service_fn!(get_service, GET);
    chained_service_fn!(head_service, HEAD);
    chained_service_fn!(options_service, OPTIONS);
    chained_service_fn!(patch_service, PATCH);
    chained_service_fn!(post_service, POST);
    chained_service_fn!(put_service, PUT);
    chained_service_fn!(trace_service, TRACE);

    #[doc = include_str!("../docs/method_routing/fallback.md")]
    pub fn fallback<S, ResBody>(mut self, svc: S) -> Self
    where
        S: Service<Request<ReqBody>, Response = Response<ResBody>, Error = E>
            + Clone
            + Send
            + 'static,
        S::Future: Send + 'static,
        ResBody: HttpBody<Data = Bytes> + Send + 'static,
        ResBody::Error: Into<BoxError>,
    {
        self.fallback = Fallback::Custom(Route::new(svc.map_response(|res| res.map(boxed))));
        self
    }

    fn fallback_boxed_response_body<S>(mut self, svc: S) -> Self
    where
        S: Service<Request<ReqBody>, Response = Response, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        self.fallback = Fallback::Custom(Route::new(svc));
        self
    }

    #[doc = include_str!("../docs/method_routing/layer.md")]
    pub fn layer<L, NewReqBody, NewResBody, NewError>(
        self,
        layer: L,
    ) -> MethodRouter<NewReqBody, NewError>
    where
        L: Layer<Route<ReqBody, E>>,
        L::Service: Service<Request<NewReqBody>, Response = Response<NewResBody>, Error = NewError>
            + Clone
            + Send
            + 'static,
        <L::Service as Service<Request<NewReqBody>>>::Future: Send + 'static,
        NewResBody: HttpBody<Data = Bytes> + Send + 'static,
        NewResBody::Error: Into<BoxError>,
    {
        let layer = ServiceBuilder::new()
            .layer_fn(Route::new)
            .layer(MapResponseBodyLayer::new(boxed))
            .layer(layer)
            .into_inner();
        let layer_fn = |s| layer.layer(s);

        MethodRouter {
            get: self.get.map(layer_fn),
            head: self.head.map(layer_fn),
            delete: self.delete.map(layer_fn),
            options: self.options.map(layer_fn),
            patch: self.patch.map(layer_fn),
            post: self.post.map(layer_fn),
            put: self.put.map(layer_fn),
            trace: self.trace.map(layer_fn),
            fallback: self.fallback.map(layer_fn),
            allow_header: self.allow_header,
            _request_body: PhantomData,
        }
    }

    #[doc = include_str!("../docs/method_routing/route_layer.md")]
    pub fn route_layer<L, NewResBody>(self, layer: L) -> MethodRouter<ReqBody, E>
    where
        L: Layer<Route<ReqBody, E>>,
        L::Service: Service<Request<ReqBody>, Response = Response<NewResBody>, Error = E>
            + Clone
            + Send
            + 'static,
        <L::Service as Service<Request<ReqBody>>>::Future: Send + 'static,
        NewResBody: HttpBody<Data = Bytes> + Send + 'static,
        NewResBody::Error: Into<BoxError>,
    {
        let layer = ServiceBuilder::new()
            .layer_fn(Route::new)
            .layer(MapResponseBodyLayer::new(boxed))
            .layer(layer)
            .into_inner();
        let layer_fn = |s| layer.layer(s);

        MethodRouter {
            get: self.get.map(layer_fn),
            head: self.head.map(layer_fn),
            delete: self.delete.map(layer_fn),
            options: self.options.map(layer_fn),
            patch: self.patch.map(layer_fn),
            post: self.post.map(layer_fn),
            put: self.put.map(layer_fn),
            trace: self.trace.map(layer_fn),
            fallback: self.fallback,
            allow_header: self.allow_header,
            _request_body: PhantomData,
        }
    }

    #[doc = include_str!("../docs/method_routing/merge.md")]
    pub fn merge(self, other: MethodRouter<ReqBody, E>) -> Self {
        macro_rules! merge {
            ( $first:ident, $second:ident ) => {
                match ($first, $second) {
                    (Some(_), Some(_)) => panic!(concat!(
                        "Overlapping method route. Cannot merge two method routes that both define `",
                        stringify!($first),
                        "`"
                    )),
                    (Some(svc), None) => Some(svc),
                    (None, Some(svc)) => Some(svc),
                    (None, None) => None,
                }
            };
        }

        let Self {
            get,
            head,
            delete,
            options,
            patch,
            post,
            put,
            trace,
            fallback,
            allow_header,
            _request_body: _,
        } = self;

        let Self {
            get: get_other,
            head: head_other,
            delete: delete_other,
            options: options_other,
            patch: patch_other,
            post: post_other,
            put: put_other,
            trace: trace_other,
            fallback: fallback_other,
            allow_header: allow_header_other,
            _request_body: _,
        } = other;

        let get = merge!(get, get_other);
        let head = merge!(head, head_other);
        let delete = merge!(delete, delete_other);
        let options = merge!(options, options_other);
        let patch = merge!(patch, patch_other);
        let post = merge!(post, post_other);
        let put = merge!(put, put_other);
        let trace = merge!(trace, trace_other);

        let fallback = match (fallback, fallback_other) {
            (pick @ Fallback::Default(_), Fallback::Default(_)) => pick,
            (Fallback::Default(_), pick @ Fallback::Custom(_)) => pick,
            (pick @ Fallback::Custom(_), Fallback::Default(_)) => pick,
            (Fallback::Custom(_), Fallback::Custom(_)) => {
                panic!("Cannot merge two `MethodRouter`s that both have a fallback")
            }
        };

        let allow_header = match (allow_header, allow_header_other) {
            (AllowHeader::Skip, _) | (_, AllowHeader::Skip) => AllowHeader::Skip,
            (AllowHeader::None, AllowHeader::None) => AllowHeader::None,
            (AllowHeader::None, AllowHeader::Bytes(pick)) => AllowHeader::Bytes(pick),
            (AllowHeader::Bytes(pick), AllowHeader::None) => AllowHeader::Bytes(pick),
            (AllowHeader::Bytes(mut a), AllowHeader::Bytes(b)) => {
                a.extend_from_slice(b",");
                a.extend_from_slice(&b);
                AllowHeader::Bytes(a)
            }
        };

        Self {
            get,
            head,
            delete,
            options,
            patch,
            post,
            put,
            trace,
            fallback,
            allow_header,
            _request_body: PhantomData,
        }
    }

    /// Apply a [`HandleErrorLayer`].
    ///
    /// This is a convenience method for doing `self.layer(HandleErrorLayer::new(f))`.
    pub fn handle_error<F, T>(self, f: F) -> MethodRouter<ReqBody, Infallible>
    where
        F: Clone + Send + 'static,
        HandleError<Route<ReqBody, E>, F, T>:
            Service<Request<ReqBody>, Response = Response, Error = Infallible>,
        <HandleError<Route<ReqBody, E>, F, T> as Service<Request<ReqBody>>>::Future: Send,
        T: 'static,
        E: 'static,
        ReqBody: 'static,
    {
        self.layer(HandleErrorLayer::new(f))
    }

    fn on_service_boxed_response_body<S>(self, filter: MethodFilter, svc: S) -> Self
    where
        S: Service<Request<ReqBody>, Response = Response, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        // written with a pattern match like this to ensure we update all fields
        let Self {
            mut get,
            mut head,
            mut delete,
            mut options,
            mut patch,
            mut post,
            mut put,
            mut trace,
            fallback,
            mut allow_header,
            _request_body: _,
        } = self;
        let svc = Some(Route::new(svc));
        if filter.contains(MethodFilter::GET) {
            get = svc.clone();
            append_allow_header(&mut allow_header, "GET");
            append_allow_header(&mut allow_header, "HEAD");
        }
        if filter.contains(MethodFilter::HEAD) {
            append_allow_header(&mut allow_header, "HEAD");
            head = svc.clone();
        }
        if filter.contains(MethodFilter::DELETE) {
            append_allow_header(&mut allow_header, "DELETE");
            delete = svc.clone();
        }
        if filter.contains(MethodFilter::OPTIONS) {
            append_allow_header(&mut allow_header, "OPTIONS");
            options = svc.clone();
        }
        if filter.contains(MethodFilter::PATCH) {
            append_allow_header(&mut allow_header, "PATCH");
            patch = svc.clone();
        }
        if filter.contains(MethodFilter::POST) {
            append_allow_header(&mut allow_header, "POST");
            post = svc.clone();
        }
        if filter.contains(MethodFilter::PUT) {
            append_allow_header(&mut allow_header, "PUT");
            put = svc.clone();
        }
        if filter.contains(MethodFilter::TRACE) {
            append_allow_header(&mut allow_header, "TRACE");
            trace = svc;
        }
        Self {
            get,
            head,
            delete,
            options,
            patch,
            post,
            put,
            trace,
            fallback,
            allow_header,
            _request_body: PhantomData,
        }
    }

    fn skip_allow_header(mut self) -> Self {
        self.allow_header = AllowHeader::Skip;
        self
    }
}

fn append_allow_header(allow_header: &mut AllowHeader, method: &'static str) {
    match allow_header {
        AllowHeader::None => {
            *allow_header = AllowHeader::Bytes(BytesMut::from(method));
        }
        AllowHeader::Skip => {}
        AllowHeader::Bytes(allow_header) => {
            if let Ok(s) = std::str::from_utf8(allow_header) {
                if !s.contains(method) {
                    allow_header.extend_from_slice(b",");
                    allow_header.extend_from_slice(method.as_bytes());
                }
            } else {
                #[cfg(debug_assertions)]
                panic!("`allow_header` contained invalid uft-8. This should never happen")
            }
        }
    }
}

impl<B, E> Clone for MethodRouter<B, E> {
    fn clone(&self) -> Self {
        Self {
            get: self.get.clone(),
            head: self.head.clone(),
            delete: self.delete.clone(),
            options: self.options.clone(),
            patch: self.patch.clone(),
            post: self.post.clone(),
            put: self.put.clone(),
            trace: self.trace.clone(),
            fallback: self.fallback.clone(),
            allow_header: self.allow_header.clone(),
            _request_body: PhantomData,
        }
    }
}

impl<B, E> Default for MethodRouter<B, E>
where
    B: Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

use crate::routing::future::RouteFuture;

impl<B, E> Service<Request<B>> for MethodRouter<B, E>
where
    B: HttpBody,
{
    type Response = Response;
    type Error = E;
    type Future = RouteFuture<B, E>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        macro_rules! call {
            (
                $req:expr,
                $method:expr,
                $method_variant:ident,
                $svc:expr
            ) => {
                if $method == Method::$method_variant {
                    if let Some(svc) = $svc {
                        return RouteFuture::from_future(svc.oneshot_inner($req))
                            .strip_body($method == Method::HEAD);
                    }
                }
            };
        }

        let method = req.method().clone();

        // written with a pattern match like this to ensure we call all routes
        let Self {
            get,
            head,
            delete,
            options,
            patch,
            post,
            put,
            trace,
            fallback,
            allow_header,
            _request_body: _,
        } = self;

        call!(req, method, HEAD, head);
        call!(req, method, HEAD, get);
        call!(req, method, GET, get);
        call!(req, method, POST, post);
        call!(req, method, OPTIONS, options);
        call!(req, method, PATCH, patch);
        call!(req, method, PUT, put);
        call!(req, method, DELETE, delete);
        call!(req, method, TRACE, trace);

        let future = match fallback {
            Fallback::Default(fallback) => RouteFuture::from_future(fallback.oneshot_inner(req))
                .strip_body(method == Method::HEAD),
            Fallback::Custom(fallback) => RouteFuture::from_future(fallback.oneshot_inner(req))
                .strip_body(method == Method::HEAD),
        };

        match allow_header {
            AllowHeader::None => future.allow_header(Bytes::new()),
            AllowHeader::Skip => future,
            AllowHeader::Bytes(allow_header) => future.allow_header(allow_header.clone().freeze()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{body::Body, error_handling::HandleErrorLayer};
    use axum_core::response::IntoResponse;
    use http::{header::ALLOW, HeaderMap};
    use std::time::Duration;
    use tower::{timeout::TimeoutLayer, Service, ServiceExt};
    use tower_http::{auth::RequireAuthorizationLayer, services::fs::ServeDir};

    #[tokio::test]
    async fn method_not_allowed_by_default() {
        let mut svc = MethodRouter::new();
        let (status, _, body) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn get_handler() {
        let mut svc = MethodRouter::new().get(ok);
        let (status, _, body) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "ok");
    }

    #[tokio::test]
    async fn get_accepts_head() {
        let mut svc = MethodRouter::new().get(ok);
        let (status, _, body) = call(Method::HEAD, &mut svc).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn head_takes_precedence_over_get() {
        let mut svc = MethodRouter::new().head(created).get(ok);
        let (status, _, body) = call(Method::HEAD, &mut svc).await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn merge() {
        let mut svc = get(ok).merge(post(ok));

        let (status, _, _) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::OK);

        let (status, _, _) = call(Method::POST, &mut svc).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn layer() {
        let mut svc = MethodRouter::new()
            .get(|| async { std::future::pending::<()>().await })
            .layer(RequireAuthorizationLayer::bearer("password"));

        // method with route
        let (status, _, _) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // method without route
        let (status, _, _) = call(Method::DELETE, &mut svc).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn route_layer() {
        let mut svc = MethodRouter::new()
            .get(|| async { std::future::pending::<()>().await })
            .route_layer(RequireAuthorizationLayer::bearer("password"));

        // method with route
        let (status, _, _) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // method without route
        let (status, _, _) = call(Method::DELETE, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    }

    #[allow(dead_code)]
    fn buiding_complex_router() {
        let app = crate::Router::new().route(
            "/",
            // use the all the things :bomb:
            get(ok)
                .post(ok)
                .route_layer(RequireAuthorizationLayer::bearer("password"))
                .merge(
                    delete_service(ServeDir::new("."))
                        .handle_error(|_| async { StatusCode::NOT_FOUND }),
                )
                .fallback((|| async { StatusCode::NOT_FOUND }).into_service())
                .put(ok)
                .layer(
                    ServiceBuilder::new()
                        .layer(HandleErrorLayer::new(|_| async {
                            StatusCode::REQUEST_TIMEOUT
                        }))
                        .layer(TimeoutLayer::new(Duration::from_secs(10))),
                ),
        );

        crate::Server::bind(&"0.0.0.0:0".parse().unwrap()).serve(app.into_make_service());
    }

    #[tokio::test]
    async fn sets_allow_header() {
        let mut svc = MethodRouter::new().put(ok).patch(ok);
        let (status, headers, _) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(headers[ALLOW], "PUT,PATCH");
    }

    #[tokio::test]
    async fn sets_allow_header_get_head() {
        let mut svc = MethodRouter::new().get(ok).head(ok);
        let (status, headers, _) = call(Method::PUT, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(headers[ALLOW], "GET,HEAD");
    }

    #[tokio::test]
    async fn empty_allow_header_by_default() {
        let mut svc = MethodRouter::new();
        let (status, headers, _) = call(Method::PATCH, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(headers[ALLOW], "");
    }

    #[tokio::test]
    async fn allow_header_when_merging() {
        let a = put(ok).patch(ok);
        let b = get(ok).head(ok);
        let mut svc = a.merge(b);

        let (status, headers, _) = call(Method::DELETE, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(headers[ALLOW], "PUT,PATCH,GET,HEAD");
    }

    #[tokio::test]
    async fn allow_header_any() {
        let mut svc = any(ok);

        let (status, headers, _) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::OK);
        assert!(!headers.contains_key(ALLOW));
    }

    #[tokio::test]
    async fn allow_header_with_fallback() {
        let mut svc = MethodRouter::new().get(ok).fallback(
            (|| async { (StatusCode::METHOD_NOT_ALLOWED, "Method not allowed") }).into_service(),
        );

        let (status, headers, _) = call(Method::DELETE, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(headers[ALLOW], "GET,HEAD");
    }

    #[tokio::test]
    async fn allow_header_with_fallback_that_sets_allow() {
        async fn fallback(method: Method) -> Response {
            if method == Method::POST {
                "OK".into_response()
            } else {
                (
                    StatusCode::METHOD_NOT_ALLOWED,
                    [(ALLOW, "GET,POST")],
                    "Method not allowed",
                )
                    .into_response()
            }
        }

        let mut svc = MethodRouter::new()
            .get(ok)
            .fallback(fallback.into_service());

        let (status, _, _) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::OK);

        let (status, _, _) = call(Method::POST, &mut svc).await;
        assert_eq!(status, StatusCode::OK);

        let (status, headers, _) = call(Method::DELETE, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(headers[ALLOW], "GET,POST");
    }

    async fn call<S>(method: Method, svc: &mut S) -> (StatusCode, HeaderMap, String)
    where
        S: Service<Request<Body>, Response = Response, Error = Infallible>,
    {
        let request = Request::builder()
            .uri("/")
            .method(method)
            .body(Body::empty())
            .unwrap();
        let response = svc.ready().await.unwrap().call(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let body = String::from_utf8(hyper::body::to_bytes(body).await.unwrap().to_vec()).unwrap();
        (parts.status, parts.headers, body)
    }

    async fn ok() -> (StatusCode, &'static str) {
        (StatusCode::OK, "ok")
    }

    async fn created() -> (StatusCode, &'static str) {
        (StatusCode::CREATED, "created")
    }
}
