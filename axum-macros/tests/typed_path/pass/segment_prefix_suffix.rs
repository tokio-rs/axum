use axum_extra::routing::TypedPath;
use serde::Deserialize;

#[derive(TypedPath, Deserialize)]
#[typed_path("/@{username}/file-{id}.json")]
struct NamedFieldsPath {
    username: String,
    id: u32,
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/@{username}/file-{id}.json")]
struct TuplePath(String, u32);

fn main() {
    _ = axum::Router::<()>::new().route("/", axum::routing::get(|_: NamedFieldsPath| async {}));
    _ = axum::Router::<()>::new().route("/", axum::routing::get(|_: TuplePath| async {}));

    assert_eq!(NamedFieldsPath::PATH, "/@{username}/file-{id}.json");
    assert_eq!(
        format!(
            "{}",
            NamedFieldsPath {
                username: "alice".to_owned(),
                id: 42,
            }
        ),
        "/@alice/file-42.json"
    );

    assert_eq!(TuplePath::PATH, "/@{username}/file-{id}.json");
    assert_eq!(
        format!("{}", TuplePath("bob".to_owned(), 7)),
        "/@bob/file-7.json"
    );
}
