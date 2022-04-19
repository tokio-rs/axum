//! Type definitions for OpenAPI descriptions of routes

use either::Either;
use indexmap::IndexMap;
use schemars::schema::RootSchema;
use crate::HandlerArg;

#[derive(serde::Serialize, Debug)]
pub struct OpenApi {
    pub(crate) openapi: &'static str,
    pub info: Info,
    pub paths: IndexMap<String, PathItem>,
}

#[derive(serde::Serialize, Debug, Default)]
pub struct Info {
    pub title: String,
    pub description: Option<String>,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PathItem {
    pub post: Option<Operation>,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub tags: &'static [&'static str],
    pub summary: &'static str,
    pub description: &'static str,
    pub operation_id: &'static str,
    pub parameters: Vec<Parameter>,
    pub request_body: Option<RequestBody>,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Parameter {
    #[serde(rename = "in")]
    pub location: ParameterLocation,

    pub name: &'static str,
    pub description: &'static str,
    pub required: bool,
    pub deprecated: bool,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum ParameterLocation {
    Query,
    Header,
    Path,
    Cookie
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RequestBody {
    pub description: &'static str,
    pub content: RequestBodyContent,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum RequestBodyContent {
    #[serde(rename = "application/json")]
    Json(RootSchema)
}

impl Operation {
    #[doc(hidden)]
    pub fn __push_handler_arg<T: HandlerArg>(&mut self) {
        match T::describe() {
            Some(Either::Left(param)) => self.parameters.push(param),
            Some(Either::Right(body)) => {
                if let Some(body) = self.request_body.replace(body) {
                    panic!("handler has more than one request body: {:?}", body)
                }
            },
            None => (),
        }
    }
}