use crate::{
    body::{Body, BoxBody},
    handler::{Handler, HandlerSvc},
    response::IntoResponse,
    App, HandleError, IntoService, ResultExt,
};
use bytes::Bytes;
use futures_util::{future, ready};
use http::{Method, Request, Response, StatusCode};
use pin_project::pin_project;
use std::{
    convert::Infallible,
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{
    buffer::{Buffer, BufferLayer},
    util::BoxService,
    BoxError, Layer, Service, ServiceBuilder,
};

#[derive(Clone, Copy)]
pub struct AlwaysNotFound(pub(crate) ());

impl<R> Service<R> for AlwaysNotFound {
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: R) -> Self::Future {
        let mut res = Response::new(Body::empty());
        *res.status_mut() = StatusCode::NOT_FOUND;
        future::ok(res)
    }
}

#[derive(Debug, Clone)]
pub struct RouteAt<R> {
    pub(crate) app: App<R>,
    pub(crate) route_spec: Bytes,
}

macro_rules! define_route_at_methods {
    (
        RouteAt:
        $name:ident,
        $svc_method_name:ident,
        $method:ident
    ) => {
        pub fn $name<F, B, T>(self, handler_fn: F) -> RouteBuilder<Or<HandlerSvc<F, B, T>, R>>
        where
            F: Handler<B, T>,
        {
            self.add_route(handler_fn, Method::$method)
        }

        pub fn $svc_method_name<S, B>(self, service: S) -> RouteBuilder<Or<S, R>>
        where
            S: Service<Request<Body>, Response = Response<B>, Error = Infallible> + Clone,
        {
            self.add_route_service(service, Method::$method)
        }
    };

    (
        RouteBuilder:
        $name:ident,
        $svc_method_name:ident,
        $method:ident
    ) => {
        pub fn $name<F, B, T>(self, handler_fn: F) -> RouteBuilder<Or<HandlerSvc<F, B, T>, R>>
        where
            F: Handler<B, T>,
        {
            self.app.at_bytes(self.route_spec).$name(handler_fn)
        }

        pub fn $svc_method_name<S, B>(self, service: S) -> RouteBuilder<Or<S, R>>
        where
            S: Service<Request<Body>, Response = Response<B>, Error = Infallible> + Clone,
        {
            self.app.at_bytes(self.route_spec).$svc_method_name(service)
        }
    };
}

impl<R> RouteAt<R> {
    define_route_at_methods!(RouteAt: get, get_service, GET);
    define_route_at_methods!(RouteAt: post, post_service, POST);
    define_route_at_methods!(RouteAt: put, put_service, PUT);
    define_route_at_methods!(RouteAt: patch, patch_service, PATCH);
    define_route_at_methods!(RouteAt: delete, delete_service, DELETE);
    define_route_at_methods!(RouteAt: head, head_service, HEAD);
    define_route_at_methods!(RouteAt: options, options_service, OPTIONS);
    define_route_at_methods!(RouteAt: connect, connect_service, CONNECT);
    define_route_at_methods!(RouteAt: trace, trace_service, TRACE);

    fn add_route<H, B, T>(
        self,
        handler: H,
        method: Method,
    ) -> RouteBuilder<Or<HandlerSvc<H, B, T>, R>>
    where
        H: Handler<B, T>,
    {
        self.add_route_service(HandlerSvc::new(handler), method)
    }

    fn add_route_service<S>(self, service: S, method: Method) -> RouteBuilder<Or<S, R>> {
        assert!(
            self.route_spec.starts_with(b"/"),
            "route spec must start with a slash (`/`)"
        );

        let new_app = App {
            service_tree: Or {
                service,
                route_spec: RouteSpec::new(method, self.route_spec.clone()),
                fallback: self.app.service_tree,
                handler_ready: false,
                fallback_ready: false,
            },
        };

        RouteBuilder {
            app: new_app,
            route_spec: self.route_spec,
        }
    }
}

pub struct RouteBuilder<R> {
    app: App<R>,
    route_spec: Bytes,
}

impl<R> Clone for RouteBuilder<R>
where
    R: Clone,
{
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            route_spec: self.route_spec.clone(),
        }
    }
}

impl<R> RouteBuilder<R> {
    fn new(app: App<R>, route_spec: impl Into<Bytes>) -> Self {
        Self {
            app,
            route_spec: route_spec.into(),
        }
    }

    pub fn at(self, route_spec: &str) -> RouteAt<R> {
        self.app.at(route_spec)
    }

    define_route_at_methods!(RouteBuilder: get, get_service, GET);
    define_route_at_methods!(RouteBuilder: post, post_service, POST);
    define_route_at_methods!(RouteBuilder: put, put_service, PUT);
    define_route_at_methods!(RouteBuilder: patch, patch_service, PATCH);
    define_route_at_methods!(RouteBuilder: delete, delete_service, DELETE);
    define_route_at_methods!(RouteBuilder: head, head_service, HEAD);
    define_route_at_methods!(RouteBuilder: options, options_service, OPTIONS);
    define_route_at_methods!(RouteBuilder: connect, connect_service, CONNECT);
    define_route_at_methods!(RouteBuilder: trace, trace_service, TRACE);

    pub fn into_service(self) -> IntoService<R> {
        IntoService {
            service_tree: self.app.service_tree,
        }
    }

    pub fn layer<L>(self, layer: L) -> RouteBuilder<L::Service>
    where
        L: Layer<R>,
    {
        let layered = layer.layer(self.app.service_tree);
        let app = App::new(layered);
        RouteBuilder::new(app, self.route_spec)
    }

    pub fn handle_error<F, B, Res>(self, f: F) -> RouteBuilder<HandleError<R, F, R::Error>>
    where
        R: Service<Request<Body>, Response = Response<B>>,
        F: FnOnce(R::Error) -> Res,
        Res: IntoResponse<Body>,
        B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        B::Error: Into<BoxError> + Send + Sync + 'static,
    {
        let svc = HandleError::new(self.app.service_tree, f);
        let app = App::new(svc);
        RouteBuilder::new(app, self.route_spec)
    }

    pub fn boxed<B>(self) -> RouteBuilder<BoxServiceTree<B>>
    where
        R: Service<Request<Body>, Response = Response<B>, Error = Infallible> + Send + 'static,
        R::Future: Send,
        B: From<String> + 'static,
    {
        let svc = ServiceBuilder::new()
            .layer(BufferLayer::new(1024))
            .layer(BoxService::layer())
            .service(self.app.service_tree);

        let app = App::new(BoxServiceTree {
            inner: svc,
            poll_ready_error: None,
        });
        RouteBuilder::new(app, self.route_spec)
    }
}

pub struct Or<H, F> {
    service: H,
    route_spec: RouteSpec,
    fallback: F,
    handler_ready: bool,
    fallback_ready: bool,
}

impl<H, F> Clone for Or<H, F>
where
    H: Clone,
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            service: self.service.clone(),
            fallback: self.fallback.clone(),
            route_spec: self.route_spec.clone(),
            // important to reset readiness when cloning
            handler_ready: false,
            fallback_ready: false,
        }
    }
}

#[derive(Clone)]
struct RouteSpec {
    method: Method,
    spec: Bytes,
}

impl RouteSpec {
    fn new(method: Method, spec: impl Into<Bytes>) -> Self {
        Self {
            method,
            spec: spec.into(),
        }
    }
}

impl RouteSpec {
    fn matches<B>(&self, req: &Request<B>) -> Option<Vec<(String, String)>> {
        if req.method() != self.method {
            return None;
        }

        let path = req.uri().path().as_bytes();
        let path_parts = path.split(|b| *b == b'/');

        let spec_parts = self.spec.split(|b| *b == b'/');

        if spec_parts.clone().count() != path_parts.clone().count() {
            return None;
        }

        let mut params = Vec::new();

        spec_parts
            .zip(path_parts)
            .all(|(spec, path)| {
                if let Some(key) = spec.strip_prefix(b":") {
                    let key = std::str::from_utf8(key).unwrap().to_string();
                    if let Ok(value) = std::str::from_utf8(path) {
                        params.push((key, value.to_string()));
                        true
                    } else {
                        false
                    }
                } else {
                    spec == path
                }
            })
            .then(|| params)
    }
}

impl<H, F, HB, FB> Service<Request<Body>> for Or<H, F>
where
    H: Service<Request<Body>, Response = Response<HB>, Error = Infallible>,
    HB: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    HB::Error: Into<BoxError>,

    F: Service<Request<Body>, Response = Response<FB>, Error = Infallible>,
    FB: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    FB::Error: Into<BoxError>,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = future::Either<BoxResponseBody<H::Future>, BoxResponseBody<F::Future>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        loop {
            if !self.handler_ready {
                ready!(self.service.poll_ready(cx)).unwrap_infallible();
                self.handler_ready = true;
            }

            if !self.fallback_ready {
                ready!(self.fallback.poll_ready(cx)).unwrap_infallible();
                self.fallback_ready = true;
            }

            if self.handler_ready && self.fallback_ready {
                return Poll::Ready(Ok(()));
            }
        }
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        if let Some(params) = self.route_spec.matches(&req) {
            assert!(
                self.handler_ready,
                "handler not ready. Did you forget to call `poll_ready`?"
            );

            self.handler_ready = false;

            req.extensions_mut().insert(Some(UrlParams(params)));

            future::Either::Left(BoxResponseBody(self.service.call(req)))
        } else {
            assert!(
                self.fallback_ready,
                "fallback not ready. Did you forget to call `poll_ready`?"
            );

            self.fallback_ready = false;

            // TODO(david): this leads to each route creating one box body, probably not great
            future::Either::Right(BoxResponseBody(self.fallback.call(req)))
        }
    }
}

pub(crate) struct UrlParams(pub(crate) Vec<(String, String)>);

#[pin_project]
pub struct BoxResponseBody<F>(#[pin] F);

impl<F, B> Future for BoxResponseBody<F>
where
    F: Future<Output = Result<Response<B>, Infallible>>,
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError>,
{
    type Output = Result<Response<BoxBody>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let response: Response<B> = ready!(self.project().0.poll(cx)).unwrap_infallible();
        let response = response.map(|body| {
            let body = body.map_err(Into::into);
            BoxBody::new(body)
        });
        Poll::Ready(Ok(response))
    }
}

pub struct BoxServiceTree<B> {
    inner: Buffer<BoxService<Request<Body>, Response<B>, Infallible>, Request<Body>>,
    poll_ready_error: Option<BoxError>,
}

impl<B> Clone for BoxServiceTree<B> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            poll_ready_error: None,
        }
    }
}

impl<B> fmt::Debug for BoxServiceTree<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxServiceTree").finish()
    }
}

impl<B> Service<Request<Body>> for BoxServiceTree<B>
where
    B: From<String> + 'static,
{
    type Response = Response<B>;
    type Error = Infallible;
    type Future = BoxServiceTreeResponseFuture<B>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO(david): downcast this into one of the cases in `tower::buffer::error`
        // and convert the error into a response. `ServiceError` should never be able to happen
        // since all inner services use `Infallible` as the error type.
        match ready!(self.inner.poll_ready(cx)) {
            Ok(_) => Poll::Ready(Ok(())),
            Err(err) => {
                self.poll_ready_error = Some(err);
                Poll::Ready(Ok(()))
            }
        }
    }

    #[inline]
    fn call(&mut self, req: Request<Body>) -> Self::Future {
        if let Some(err) = self.poll_ready_error.take() {
            return BoxServiceTreeResponseFuture {
                kind: Kind::Response(Some(handle_buffer_error(err))),
            };
        }

        BoxServiceTreeResponseFuture {
            kind: Kind::Future(self.inner.call(req)),
        }
    }
}

#[pin_project]
pub struct BoxServiceTreeResponseFuture<B> {
    #[pin]
    kind: Kind<B>,
}

#[pin_project(project = KindProj)]
enum Kind<B> {
    Response(Option<Response<B>>),
    Future(#[pin] InnerFuture<B>),
}

type InnerFuture<B> = tower::buffer::future::ResponseFuture<
    Pin<Box<dyn Future<Output = Result<Response<B>, Infallible>> + Send + 'static>>,
>;

impl<B> Future for BoxServiceTreeResponseFuture<B>
where
    B: From<String>,
{
    type Output = Result<Response<B>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().kind.project() {
            KindProj::Response(res) => Poll::Ready(Ok(res.take().unwrap())),
            KindProj::Future(future) => match ready!(future.poll(cx)) {
                Ok(res) => Poll::Ready(Ok(res)),
                Err(err) => Poll::Ready(Ok(handle_buffer_error(err))),
            },
        }
    }
}

fn handle_buffer_error<B>(error: BoxError) -> Response<B>
where
    B: From<String>,
{
    use tower::buffer::error::{Closed, ServiceError};

    let error = match error.downcast::<Closed>() {
        Ok(closed) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(B::from(closed.to_string()))
                .unwrap();
        }
        Err(e) => e,
    };

    let error = match error.downcast::<ServiceError>() {
        Ok(service_error) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(B::from(format!("Service error: {}. This is a bug in tower-web. All inner services should be infallible. Please file an issue", service_error)))
                .unwrap();
        }
        Err(e) => e,
    };

    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(B::from(format!(
            "Uncountered an unknown error: {}. This should never happen. Please file an issue",
            error
        )))
        .unwrap()
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_routing() {
        assert_match((Method::GET, "/"), (Method::GET, "/"));
        refute_match((Method::GET, "/"), (Method::POST, "/"));
        refute_match((Method::POST, "/"), (Method::GET, "/"));

        assert_match((Method::GET, "/foo"), (Method::GET, "/foo"));
        assert_match((Method::GET, "/foo/"), (Method::GET, "/foo/"));
        refute_match((Method::GET, "/foo"), (Method::GET, "/foo/"));
        refute_match((Method::GET, "/foo/"), (Method::GET, "/foo"));

        assert_match((Method::GET, "/foo/bar"), (Method::GET, "/foo/bar"));
        refute_match((Method::GET, "/foo/bar/"), (Method::GET, "/foo/bar"));
        refute_match((Method::GET, "/foo/bar"), (Method::GET, "/foo/bar/"));

        assert_match((Method::GET, "/:value"), (Method::GET, "/foo"));
        assert_match((Method::GET, "/users/:id"), (Method::GET, "/users/1"));
        assert_match(
            (Method::GET, "/users/:id/action"),
            (Method::GET, "/users/42/action"),
        );
        refute_match(
            (Method::GET, "/users/:id/action"),
            (Method::GET, "/users/42"),
        );
        refute_match(
            (Method::GET, "/users/:id"),
            (Method::GET, "/users/42/action"),
        );
    }

    fn assert_match(route_spec: (Method, &'static str), req_spec: (Method, &'static str)) {
        let route = RouteSpec::new(route_spec.0.clone(), route_spec.1);
        let req = Request::builder()
            .method(req_spec.0.clone())
            .uri(req_spec.1)
            .body(())
            .unwrap();

        assert!(
            route.matches(&req).is_some(),
            "`{} {}` doesn't match `{} {}`",
            req.method(),
            req.uri().path(),
            route.method,
            std::str::from_utf8(&route.spec).unwrap(),
        );
    }

    fn refute_match(route_spec: (Method, &'static str), req_spec: (Method, &'static str)) {
        let route = RouteSpec::new(route_spec.0.clone(), route_spec.1);
        let req = Request::builder()
            .method(req_spec.0.clone())
            .uri(req_spec.1)
            .body(())
            .unwrap();

        assert!(
            route.matches(&req).is_none(),
            "`{} {}` shouldn't match `{} {}`",
            req.method(),
            req.uri().path(),
            route.method,
            std::str::from_utf8(&route.spec).unwrap(),
        );
    }
}
