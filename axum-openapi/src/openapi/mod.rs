//! Type definitions for OpenAPI descriptions of routes

use either::Either;
use indexmap::IndexMap;
use schemars::schema::RootSchema;
use serde::{Serialize, Serializer};
use serde::ser::SerializeMap;
use axum::http::StatusCode;

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

#[derive(serde::Serialize, Debug, Default)]
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
    pub responses: Responses,
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
    pub description: String,
    pub content: RequestBodyContent,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum RequestBodyContent {
    #[serde(rename = "application/json")]
    Json(RootSchema)
}

#[derive(Debug, Default)]
pub struct Responses {
    pub default: Option<Response>,
    pub responses: IndexMap<StatusCode, Response>,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Response {

}

impl Serialize for Responses {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let output_len = self.default.is_some() as usize + self.responses.len();

        let mut out = ser.serialize_map(output_len.into())?;
        if let Some(default) = &self.default {
            out.serialize_entry("default", default)?;
        }

        for (code, response) in &self.responses {
            out.serialize_entry(&code.as_u16(), response)?;
        }

        out.end()
    }
}
