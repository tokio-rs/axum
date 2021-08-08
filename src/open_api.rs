#![allow(warnings)]

use crate::{
    extract, handler,
    response::IntoResponse,
    routing::{EmptyRouter, MethodFilter, Route},
};
use indexmap::IndexMap;
use openapiv3::{
    Encoding, MediaType, NumberType, ObjectType, OpenAPI, Operation, Parameter, ParameterData,
    ParameterSchemaOrContent, PathItem, QueryStyle, ReferenceOr, RequestBody, Response, Responses,
    Schema, SchemaData, SchemaKind, StringType, Type,
};
use std::future::Future;

pub fn to_open_api<S>(svc: &S) -> OpenAPI
where
    S: ToOpenApi,
{
    let mut open_api = OpenAPI::default();
    svc.to_open_api(&mut open_api);
    open_api
}

pub trait ToOpenApi {
    fn to_open_api(&self, open_api: &mut OpenAPI);
}

pub trait ToPathItem {
    fn to_path_item(&self, path_item: &mut PathItem);
}

pub trait ToOperation<In> {
    fn to_operation(&self, operation: &mut Operation);
}

pub trait ToOperationInput {
    fn to_parameter() -> Option<Parameter> {
        None
    }

    fn to_request_body() -> Option<RequestBody> {
        None
    }
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
    fn to_response(response: &mut Response);
}

impl<S, F> ToOpenApi for Route<S, F>
where
    S: ToPathItem,
    F: ToOpenApi,
{
    fn to_open_api(&self, open_api: &mut OpenAPI) {
        let mut path_item = PathItem::default();
        self.svc.to_path_item(&mut path_item);
        open_api.paths.insert(
            self.pattern.original_pattern().to_string(),
            ReferenceOr::Item(path_item),
        );

        self.fallback.to_open_api(open_api);
    }
}

impl<E> ToOpenApi for EmptyRouter<E> {
    fn to_open_api(&self, open_api: &mut OpenAPI) {}
}

impl<E> ToPathItem for EmptyRouter<E> {
    fn to_path_item(&self, path_item: &mut PathItem) {}
}

impl<H, B, T, F> ToPathItem for handler::OnMethod<handler::IntoService<H, B, T>, F>
where
    H: handler::Handler<B, T> + ToOperation<T>,
    H::Response: ToResponse,
    F: ToPathItem,
{
    fn to_path_item(&self, path_item: &mut PathItem) {
        let mut operation = Operation::default();
        // TODO(david): operation_id
        self.svc.to_operation(&mut operation);

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

        self.fallback.to_path_item(path_item);
    }
}

impl<H, B, T> ToOperation<T> for handler::IntoService<H, B, T>
where
    H: ToOperation<T>,
{
    fn to_operation(&self, operation: &mut Operation) {
        self.handler.to_operation(operation)
    }
}

impl<F, Fut, Res> ToOperation<()> for F
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Res>,
    Res: ToResponse,
{
    fn to_operation(&self, operation: &mut Operation) {
        let mut response = Response::default();
        Res::to_response(&mut response);
        operation.responses.default = Some(ReferenceOr::Item(response));
    }
}

impl<F, Fut, Res, T1> ToOperation<(T1,)> for F
where
    F: FnOnce(T1) -> Fut,
    Fut: Future<Output = Res>,
    Res: ToResponse,
    T1: ToOperationInput,
{
    fn to_operation(&self, operation: &mut Operation) {
        let mut response = Response::default();
        Res::to_response(&mut response);
        operation.responses.default = Some(ReferenceOr::Item(response));

        if let Some(parameter) = T1::to_parameter() {
            operation.parameters = vec![ReferenceOr::Item(parameter)];
        }

        if let Some(request_body) = T1::to_request_body() {
            operation.request_body = Some(ReferenceOr::Item(request_body));
        }
    }
}

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
    fn to_response(response: &mut Response) {
        response.description = "Always empty".to_string();
    }
}

impl ToResponse for &str {
    fn to_response(response: &mut Response) {
        response.description = "Plain text".to_string();

        let mut media_type = MediaType::default();
        let schema = Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Type(Type::String(StringType::default())),
        };
        let mut encoding = Encoding::default();
        encoding.content_type = Some("text/plain".to_string());
        media_type
            .encoding
            .insert("text/plain".to_string(), encoding);
        media_type.schema = Some(ReferenceOr::Item(schema));

        response
            .content
            .insert("text/plain".to_string(), media_type);
    }
}
