use axum_extra::routing::TypedPath;
use serde::Deserialize;

// Named fields with prefix
#[derive(TypedPath, Deserialize)]
#[typed_path("/@{username}")]
struct AtUsername {
    username: String,
}

// Named fields with suffix
#[derive(TypedPath, Deserialize)]
#[typed_path("/avatars/{id}.png")]
struct AvatarPath {
    id: u32,
}

// Named fields with prefix and suffix
#[derive(TypedPath, Deserialize)]
#[typed_path("/files/{name}.tar.gz")]
struct FilePath {
    name: String,
}

// Tuple struct with prefix
#[derive(TypedPath, Deserialize)]
#[typed_path("/@{username}")]
struct AtUsernameTuple(String);

fn main() {
    _ = axum::Router::<()>::new().route("/", axum::routing::get(|_: AtUsername| async {}));
    _ = axum::Router::<()>::new().route("/", axum::routing::get(|_: AvatarPath| async {}));
    _ = axum::Router::<()>::new().route("/", axum::routing::get(|_: FilePath| async {}));
    _ = axum::Router::<()>::new().route("/", axum::routing::get(|_: AtUsernameTuple| async {}));

    assert_eq!(AtUsername::PATH, "/@{username}");
    assert_eq!(
        format!("{}", AtUsername { username: "alice".to_owned() }),
        "/@alice"
    );

    assert_eq!(AvatarPath::PATH, "/avatars/{id}.png");
    assert_eq!(format!("{}", AvatarPath { id: 42 }), "/avatars/42.png");

    assert_eq!(FilePath::PATH, "/files/{name}.tar.gz");
    assert_eq!(
        format!("{}", FilePath { name: "backup".to_owned() }),
        "/files/backup.tar.gz"
    );

    assert_eq!(AtUsernameTuple::PATH, "/@{username}");
    assert_eq!(
        format!("{}", AtUsernameTuple("bob".to_owned())),
        "/@bob"
    );
}
