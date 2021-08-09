#![allow(warnings)]

use crate::{
    body::BoxBody,
    extract, handler,
    response::{Html, IntoResponse},
    routing::{or::Or, EmptyRouter, MethodFilter, Nested, Route, RoutingDsl},
    Json,
};
use async_trait::async_trait;
use indexmap::IndexMap;
use openapiv3::{
    Encoding, MediaType, NumberType, ObjectType, OpenAPI, Operation, Parameter, ParameterData,
    ParameterSchemaOrContent, PathItem, QueryStyle, ReferenceOr, RequestBody, Response, Responses,
    Schema, SchemaData, SchemaKind, StringType, Type,
};
use std::{
    any::TypeId,
    future::Future,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::Arc,
    task::{Context, Poll},
};
use tower::Service;

pub fn to_open_api<S>(svc: &S) -> OpenAPI
where
    S: DescribePaths,
{
    let mut paths = IndexMap::new();
    svc.describe_paths(&mut paths);
    let paths = paths
        .into_iter()
        .map(|(key, paths)| (key, ReferenceOr::Item(paths)))
        .collect();

    let mut open_api = OpenAPI::default();
    open_api.openapi = "3.0.0".to_string();
    open_api.paths = paths;
    open_api
}

pub trait DescribePaths {
    fn describe_paths(&self, paths: &mut IndexMap<String, PathItem>);
}

pub trait DescribePathItem {
    fn describe_path_item(&self, path_item: &mut PathItem);
}

pub trait ToOperation<In> {
    fn to_operation(&self) -> Operation;
}

pub trait ToOperationInput {
    fn to_parameter() -> Option<Parameter> {
        None
    }

    fn to_request_body() -> Option<RequestBody> {
        None
    }
}

pub trait ToSchema {
    fn to_schema() -> Schema;
}

pub struct Query {
    pub parameter_data: ParameterData,
    pub allow_reserved: bool,
    pub style: QueryStyle,
    pub allow_empty_value: Option<bool>,
}

pub trait ToQueryParameter {
    fn to_query_parameter() -> Query;
}

pub trait ToResponse {
    fn to_response() -> Response;
}

impl<S, F> DescribePaths for Route<S, F>
where
    S: DescribePathItem,
    F: DescribePaths,
{
    fn describe_paths(&self, paths: &mut IndexMap<String, PathItem>) {
        let mut path_item = PathItem::default();
        self.svc.describe_path_item(&mut path_item);

        paths.insert(self.pattern.original_pattern().to_string(), path_item);

        self.fallback.describe_paths(paths);
    }
}

impl<A, B> DescribePaths for Or<A, B>
where
    A: DescribePaths,
    B: DescribePaths,
{
    fn describe_paths(&self, paths: &mut IndexMap<String, PathItem>) {
        self.first.describe_paths(paths);
        self.second.describe_paths(paths);
    }
}

impl<E> DescribePaths for EmptyRouter<E> {
    fn describe_paths(&self, paths: &mut IndexMap<String, PathItem>) {}
}

impl<E> DescribePathItem for EmptyRouter<E> {
    fn describe_path_item(&self, path_item: &mut PathItem) {}
}

impl<S, F> DescribePaths for Nested<S, F>
where
    S: DescribePaths,
    F: DescribePaths,
{
    fn describe_paths(&self, paths: &mut IndexMap<String, PathItem>) {
        let mut nested_paths = IndexMap::new();
        self.svc.describe_paths(&mut nested_paths);

        let nested_paths = nested_paths.into_iter().map(|(path, item)| {
            let path = format!("{}{}", self.pattern.original_pattern(), path);
            (path, item)
        });

        paths.extend(nested_paths);

        self.fallback.describe_paths(paths);
    }
}

#[derive(Clone, Debug)]
pub struct WithPaths<T> {
    pub(crate) inner: T,
    pub(crate) paths: Arc<IndexMap<String, PathItem>>,
}

impl<T> WithPaths<T> {
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> Deref for WithPaths<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for WithPaths<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T> DescribePaths for WithPaths<T> {
    fn describe_paths(&self, paths: &mut IndexMap<String, PathItem>) {
        paths.extend((&*self.paths).clone());
    }
}

impl<T> RoutingDsl for WithPaths<T> where T: RoutingDsl {}

impl<T> crate::sealed::Sealed for WithPaths<T> where T: crate::sealed::Sealed {}

impl<R, T> Service<R> for WithPaths<T>
where
    T: Service<R>,
{
    type Response = T::Response;
    type Error = T::Error;
    type Future = T::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: R) -> Self::Future {
        self.inner.call(req)
    }
}

impl ToSchema for usize {
    fn to_schema() -> Schema {
        let limit_schema_data = SchemaData {
            nullable: false,
            ..Default::default()
        };
        Schema {
            schema_data: limit_schema_data,
            schema_kind: SchemaKind::Type(Type::Number(NumberType::default())),
        }
    }
}

impl ToSchema for &str {
    fn to_schema() -> Schema {
        Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Type(Type::String(StringType::default())),
        }
    }
}

impl<T> ToSchema for Option<T>
where
    T: ToSchema,
{
    fn to_schema() -> Schema {
        let mut schema = T::to_schema();
        schema.schema_data.nullable = true;
        schema
    }
}

impl<H, B, T, F> DescribePathItem for handler::OnMethod<handler::IntoService<H, B, T>, F>
where
    H: handler::Handler<B, T> + ToOperation<T>,
    H::Response: ToResponse,
    F: DescribePathItem,
{
    fn describe_path_item(&self, path_item: &mut PathItem) {
        let mut operation = self.svc.to_operation();

        // CONNECT not supported

        if self.method.contains(MethodFilter::DELETE) {
            path_item.delete = Some(operation.clone());
        }

        if self.method.contains(MethodFilter::GET) {
            path_item.get = Some(operation.clone());
        }

        if self.method.contains(MethodFilter::HEAD) {
            path_item.head = Some(operation.clone());
        }

        if self.method.contains(MethodFilter::OPTIONS) {
            path_item.options = Some(operation.clone());
        }

        if self.method.contains(MethodFilter::PATCH) {
            path_item.patch = Some(operation.clone());
        }

        if self.method.contains(MethodFilter::POST) {
            path_item.post = Some(operation.clone());
        }

        if self.method.contains(MethodFilter::PUT) {
            path_item.put = Some(operation.clone());
        }

        if self.method.contains(MethodFilter::TRACE) {
            path_item.trace = Some(operation);
        }

        self.fallback.describe_path_item(path_item);
    }
}

impl<H, B, T> ToOperation<T> for handler::IntoService<H, B, T>
where
    H: ToOperation<T>,
{
    fn to_operation(&self) -> Operation {
        self.handler.to_operation()
    }
}

// --- MapOperationHandler

pub struct MapOperationHandler<H, F, B> {
    pub(crate) handler: H,
    pub(crate) f: F,
    pub(crate) _marker: PhantomData<fn() -> B>,
}

impl<H, F, B> Clone for MapOperationHandler<H, F, B>
where
    H: Clone,
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            f: self.f.clone(),
            _marker: self._marker,
        }
    }
}

#[async_trait]
impl<B, T, H, F> handler::Handler<B, T> for MapOperationHandler<H, F, B>
where
    H: handler::Handler<B, T> + Send + 'static,
    B: Send + 'static,
    F: Send + 'static,
{
    type Sealed = handler::sealed::Hidden;
    type Response = H::Response;

    async fn call(self, req: http::Request<B>) -> http::Response<BoxBody> {
        self.handler.call(req).await
    }
}

impl<T, H, F, B> ToOperation<T> for MapOperationHandler<H, F, B>
where
    H: ToOperation<T>,
    // TODO(david): make `MapOperation` trait so users can compose map operations without
    // dealing with functions
    F: Fn(openapiv3::Operation) -> openapiv3::Operation,
{
    fn to_operation(&self) -> Operation {
        let operation = self.handler.to_operation();
        (self.f)(operation)
    }
}

// --- OperationIdHandler

pub struct OperationIdHandler<H, B> {
    pub(crate) handler: H,
    pub(crate) id: &'static str,
    pub(crate) _marker: PhantomData<fn() -> B>,
}

impl<H, B> Clone for OperationIdHandler<H, B>
where
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            id: self.id,
            _marker: self._marker,
        }
    }
}

#[async_trait]
impl<B, T, H> handler::Handler<B, T> for OperationIdHandler<H, B>
where
    H: handler::Handler<B, T> + Send + 'static,
    B: Send + 'static,
{
    type Sealed = handler::sealed::Hidden;
    type Response = H::Response;

    async fn call(self, req: http::Request<B>) -> http::Response<BoxBody> {
        self.handler.call(req).await
    }
}

impl<T, H, B> ToOperation<T> for OperationIdHandler<H, B>
where
    H: ToOperation<T>,
{
    fn to_operation(&self) -> Operation {
        let mut operation = self.handler.to_operation();
        operation.operation_id = Some(self.id.to_string());
        operation
    }
}

impl<F, Fut, Res> ToOperation<()> for F
where
    F: FnOnce() -> Fut + 'static,
    Fut: Future<Output = Res>,
    Res: ToResponse,
{
    fn to_operation(&self) -> Operation {
        let mut operation = Operation::default();

        let response = Res::to_response();
        operation.responses.default = Some(ReferenceOr::Item(response));

        operation
    }
}

macro_rules! impl_to_operation {
    () => {};

    ( $head:ident, $($tail:ident),* $(,)? ) => {
        #[allow(non_snake_case)]
        impl<F, Fut, Res, $head, $($tail,)*> ToOperation<($head, $($tail,)*)> for F
        where
            F: FnOnce($head, $($tail,)*) -> Fut,
            Fut: Future<Output = Res>,
            Res: ToResponse,
            $head: ToOperationInput,
            $( $tail: ToOperationInput, )*
        {
            fn to_operation(&self) -> Operation {
                let mut operation = Operation::default();

                let response = Res::to_response();
                operation.responses.default = Some(ReferenceOr::Item(response));

                if let Some(parameter) = $head::to_parameter() {
                    operation.parameters = vec![ReferenceOr::Item(parameter)];
                }

                if let Some(request_body) = $head::to_request_body() {
                    operation.request_body = Some(ReferenceOr::Item(request_body));
                }

                $(
                    if let Some(parameter) = $tail::to_parameter() {
                        operation.parameters = vec![ReferenceOr::Item(parameter)];
                    }

                    if let Some(request_body) = $tail::to_request_body() {
                        operation.request_body = Some(ReferenceOr::Item(request_body));
                    }
                )*

                operation
            }
        }

        impl_to_operation!($($tail,)*);
    };
}

impl_to_operation!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);

impl<T> ToOperationInput for Option<T>
where
    T: ToOperationInput,
{
    fn to_parameter() -> Option<Parameter> {
        T::to_parameter().map(|mut parameter| {
            match &mut parameter {
                Parameter::Query { parameter_data, .. } => {
                    parameter_data.required = false;
                }
                _ => todo!(),
            }

            parameter
        })
    }

    fn to_request_body() -> Option<RequestBody> {
        T::to_request_body().map(|mut request_body| {
            request_body.required = false;
            request_body
        })
    }
}

impl<T> ToOperationInput for extract::Query<T>
where
    T: ToQueryParameter,
{
    fn to_parameter() -> Option<Parameter> {
        let Query {
            parameter_data,
            allow_reserved,
            style,
            allow_empty_value,
        } = T::to_query_parameter();

        Some(Parameter::Query {
            parameter_data,
            allow_reserved,
            style,
            allow_empty_value,
        })
    }
}

impl ToResponse for () {
    fn to_response() -> Response {
        let mut response = Response::default();
        response.description = "Always empty".to_string();
        response
    }
}

impl ToResponse for &str {
    fn to_response() -> Response {
        let mut response = Response::default();

        response.description = "Plain text".to_string();

        let mut media_type = MediaType::default();
        let mut encoding = Encoding::default();
        encoding.content_type = Some("text/plain".to_string());
        media_type
            .encoding
            .insert("text/plain".to_string(), encoding);

        response
            .content
            .insert("text/plain".to_string(), media_type);

        response
    }
}

impl<T> ToResponse for Html<T>
where
    T: ToResponse,
{
    fn to_response() -> Response {
        let mut response = T::to_response();

        response.description = "HTML".to_string();

        response.content.clear();

        let mut media_type = MediaType::default();
        let mut encoding = Encoding::default();
        encoding.content_type = Some("text/html".to_string());
        media_type
            .encoding
            .insert("text/html".to_string(), encoding);

        response.content.insert("text/html".to_string(), media_type);

        response
    }
}
