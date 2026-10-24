use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    routing::MethodFilter,
    Router,
};
use axum_extra::routing::{RouterExt, TypedMethod, TypedPath};
use std::convert::Infallible;

struct ManualPath;

impl std::fmt::Display for ManualPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("/manual")
    }
}

impl TypedPath for ManualPath {
    const PATH: &'static str = "/manual";
}

impl TypedMethod for ManualPath {
    const METHOD: MethodFilter = MethodFilter::GET;
}

impl<S> FromRequestParts<S> for ManualPath
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        _parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self)
    }
}

async fn handler(_: ManualPath) {}

fn main() {
    let _: Router<()> = Router::new().typed_get(handler);
}
