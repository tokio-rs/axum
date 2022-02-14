use axum_extra::routing::TypedPath;
use serde::Deserialize;

#[derive(TypedPath, Deserialize)]
#[typed_path("/users/:user_id/teams/:team_id")]
struct MyPath(u32, u32);

fn main() {
    axum::Router::<axum::body::Body>::new().route("/", axum::routing::get(|_: MyPath| async {}));

    assert_eq!(MyPath::PATH, "/users/:user_id/teams/:team_id");
    assert_eq!(format!("{}", MyPath(1, 2)), "/users/1/teams/2");
}
