use axum_extra::routing::{RouterExt, TypedPath};
use serde::Deserialize;

#[derive(TypedPath, Deserialize)]
#[typed_path("/:foo")]
struct MyPathNamed<T> {
    foo: T,
}

// types with wrappers should not get any where bounds auto-generated
struct WrapperStruct<T>(T);

impl<'de, T> Deserialize<'de> for WrapperStruct<T> {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        unimplemented!()
    }
}

impl<U> std::fmt::Display for WrapperStruct<U> {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unimplemented!()
    }
}

#[derive(TypedPath)]
#[typed_path("/:foo/:bar")]
struct MyPathUnnamed<T, U>(T, WrapperStruct<U>);

impl<'de, T, U> Deserialize<'de> for MyPathUnnamed<T, U> {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        unimplemented!()
    }
}

fn main() {
    _ = axum::Router::<()>::new()
        .typed_get(|_: MyPathNamed<i32>| async {})
        .typed_post(|_: MyPathUnnamed<i32, u32>| async {})
}
