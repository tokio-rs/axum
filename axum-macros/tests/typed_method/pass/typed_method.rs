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

#[derive(TypedPath, Deserialize)]
#[typed_path("/shared/{id}")]
struct SharedPath {
    id: u32,
}

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

fn main() {
    let _: Router = Router::new().typed(get_user);
    let _: Router = Router::new().typed(get_shared);

    // A colocated endpoint remains usable with the explicit method helpers.
    let _: Router = Router::new().typed_get(get_user);
    let _: Router = Router::new().typed_put(get_user);
    let _: Router = Router::new().typed_get(get_shared);
    let _: Router = Router::new().typed_put(get_shared);

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
    assert_eq!(GetUser::METHOD, MethodFilter::GET);
    assert_eq!(GetUser::PATH, "/users/{id}");
}
