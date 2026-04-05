use axum_extra::routing::TypedPath;
use serde::Deserialize;

#[derive(TypedPath, Deserialize)]
#[typed_path("/@{username}")]
struct PrefixOnly {
    username: String,
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/{file}.json")]
struct SuffixOnly {
    file: String,
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/literal{not_a_capture")]
struct MalformedBraces;

fn main() {
    _ = axum::Router::<()>::new().route("/", axum::routing::get(|_: PrefixOnly| async {}));
    _ = axum::Router::<()>::new().route("/", axum::routing::get(|_: SuffixOnly| async {}));
    _ = axum::Router::<()>::new().route("/", axum::routing::get(|_: MalformedBraces| async {}));

    assert_eq!(format!("{}", PrefixOnly { username: "alice".into() }), "/@alice");
    assert_eq!(format!("{}", SuffixOnly { file: "report".into() }), "/report.json");
    assert_eq!(format!("{}", MalformedBraces), "/literal{not_a_capture");
}
