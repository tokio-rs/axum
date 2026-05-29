use axum_extra::routing::{RouterExt, TypedPath};
use serde::Deserialize;

#[derive(TypedPath, Deserialize)]
#[typed_path("/@{username}")]
struct PrefixedCapture {
    username: String,
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/files/{name}.json")]
struct SuffixedCapture {
    name: String,
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/users/{user_id}/teams/team-{team_id}")]
struct MixedCaptures {
    user_id: u32,
    team_id: u32,
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/@{username}")]
struct UnnamedPrefixedCapture(String);

fn main() {
    _ = axum::Router::<()>::new().typed_get(|_: PrefixedCapture| async {});
    _ = axum::Router::<()>::new().typed_get(|_: SuffixedCapture| async {});
    _ = axum::Router::<()>::new().typed_get(|_: MixedCaptures| async {});

    assert_eq!(PrefixedCapture::PATH, "/@{username}");
    assert_eq!(
        format!(
            "{}",
            PrefixedCapture {
                username: "alice".to_owned(),
            }
        ),
        "/@alice"
    );

    assert_eq!(SuffixedCapture::PATH, "/files/{name}.json");
    assert_eq!(
        format!(
            "{}",
            SuffixedCapture {
                name: "report final".to_owned(),
            }
        ),
        "/files/report%20final.json"
    );

    assert_eq!(
        format!(
            "{}",
            MixedCaptures {
                user_id: 1,
                team_id: 2,
            }
        ),
        "/users/1/teams/team-2"
    );

    assert_eq!(
        format!("{}", UnnamedPrefixedCapture("bob".to_owned())),
        "/@bob"
    );
}
