use axum::{routing::MethodFilter, Router};
use axum_extra::routing::{Get, RouterExt, TypedMethod, TypedPath};
use serde::Deserialize;

#[derive(TypedPath, TypedMethod, Deserialize)]
#[typed_path("/users/{id}")]
#[typed_method(MethodFilter::GET)]
struct GetUser {
    id: u32,
}

async fn get_user(GetUser { id: _ }: GetUser) {}

#[derive(TypedMethod, TypedPath, Deserialize)]
#[typed_path("/users/{id}/alternate")]
#[typed_method(MethodFilter::POST)]
struct AlternateOrder {
    id: u32,
}

async fn alternate_order(AlternateOrder { id: _ }: AlternateOrder) {}

#[derive(TypedPath, TypedMethod)]
#[typed_path("/unit")]
#[typed_method(MethodFilter::HEAD)]
struct MethodUnit;

async fn method_unit(_: MethodUnit) {}

#[derive(TypedPath)]
#[typed_path("/methodless-unit")]
struct MethodlessUnit;

async fn methodless_unit(_: MethodlessUnit) {}

#[derive(TypedPath, TypedMethod, Deserialize)]
#[typed_path("/tuple/{id}")]
#[typed_method(MethodFilter::PATCH)]
struct MethodTuple(u32);

async fn method_tuple(MethodTuple(_): MethodTuple) {}

#[derive(TypedPath, Deserialize)]
#[typed_path("/shared/{id}")]
struct SharedPath {
    id: u32,
}

struct MethodOnly;

fn assert_typed_method<T: TypedMethod>() {}

async fn get_shared(Get(path): Get<SharedPath>) {
    let _ = path.id;
}

async fn methodless(SharedPath { id: _ }: SharedPath) {}

async fn methodless_delete(SharedPath { id: _ }: SharedPath) {}
async fn methodless_head(SharedPath { id: _ }: SharedPath) {}
async fn methodless_options(SharedPath { id: _ }: SharedPath) {}
async fn methodless_patch(SharedPath { id: _ }: SharedPath) {}
async fn methodless_post(SharedPath { id: _ }: SharedPath) {}
async fn methodless_put(SharedPath { id: _ }: SharedPath) {}
async fn methodless_trace(SharedPath { id: _ }: SharedPath) {}
async fn methodless_connect(SharedPath { id: _ }: SharedPath) {}
async fn methodless_query(SharedPath { id: _ }: SharedPath) {}

struct ManualPath;

impl std::fmt::Display for ManualPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("/manual")
    }
}

impl TypedPath for ManualPath {
    const PATH: &'static str = "/manual";
}

impl<S> axum::extract::FromRequestParts<S> for ManualPath
where
    S: Send + Sync,
{
    type Rejection = axum::http::StatusCode;

    async fn from_request_parts(
        _parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self)
    }
}

impl axum_extra::routing::MethodlessTypedPath for ManualPath {}

async fn manual(_: ManualPath) {}

fn main() {
    let _: Router = Router::new().typed(get_user);
    let _: Router = Router::new().typed(alternate_order);
    let _: Router = Router::new().typed(method_unit);
    let _: Router = Router::new().typed(method_tuple);
    let _: Router = Router::new().typed(get_shared);
    assert_typed_method::<Get<MethodOnly>>();
    let _: Router = Router::new().typed_get(methodless);
    let _: Router = Router::new().typed_delete(methodless_delete);
    let _: Router = Router::new().typed_head(methodless_head);
    let _: Router = Router::new().typed_options(methodless_options);
    let _: Router = Router::new().typed_patch(methodless_patch);
    let _: Router = Router::new().typed_post(methodless_post);
    let _: Router = Router::new().typed_put(methodless_put);
    let _: Router = Router::new().typed_trace(methodless_trace);
    let _: Router = Router::new().typed_connect(methodless_connect);
    let _: Router = Router::new().typed_query(methodless_query);
    let _: Router = Router::new().typed_head(methodless_unit);
    let _: Router = Router::new().typed_get(manual);
    assert_eq!(GetUser::METHOD, MethodFilter::GET);
    assert_eq!(GetUser::PATH, "/users/{id}");
}
