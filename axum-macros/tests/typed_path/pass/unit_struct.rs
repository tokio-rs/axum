use axum_extra::routing::TypedPath;

#[derive(TypedPath)]
#[typed_path("/users")]
struct MyPath;

fn main() {
    _ = axum::Router::<()>::new().route("/", axum::routing::get(|_: MyPath| async {}));

    assert_eq!(MyPath::PATH, "/users");
    assert_eq!(format!("{}", MyPath), "/users");
}
