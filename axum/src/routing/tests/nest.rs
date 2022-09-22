use super::*;
use crate::{body::boxed, extract::Extension};
use std::collections::HashMap;
use tower_http::services::ServeDir;

#[tokio::test]
async fn nesting_apps() {
    let api_routes = Router::new()
        .route(
            "/users",
            get(|| async { "users#index" }).post(|| async { "users#create" }),
        )
        .route(
            "/users/:id",
            get(
                |params: extract::Path<HashMap<String, String>>| async move {
                    format!(
                        "{}: users#show ({})",
                        params.get("version").unwrap(),
                        params.get("id").unwrap()
                    )
                },
            ),
        )
        .route(
            "/games/:id",
            get(
                |params: extract::Path<HashMap<String, String>>| async move {
                    format!(
                        "{}: games#show ({})",
                        params.get("version").unwrap(),
                        params.get("id").unwrap()
                    )
                },
            ),
        );

    let app = Router::new()
        .route("/", get(|| async { "hi" }))
        .nest("/:version/api", api_routes.into_service());

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "hi");

    let res = client.get("/v0/api/users").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "users#index");

    let res = client.get("/v0/api/users/123").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "v0: users#show (123)");

    let res = client.get("/v0/api/games/123").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "v0: games#show (123)");
}

#[tokio::test]
async fn wrong_method_nest() {
    let nested_app = Router::new().route("/", get(|| async {}));
    let app = Router::new().nest("/", nested_app.into_service());

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.post("/").send().await;
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);

    let res = client.patch("/foo").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn nesting_router_at_root() {
    let nested = Router::new().route("/foo", get(|uri: Uri| async move { uri.to_string() }));
    let app = Router::new().nest("/", nested.into_service());

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = client.get("/foo").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/foo");

    let res = client.get("/foo/bar").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn nesting_router_at_empty_path() {
    let nested = Router::new().route("/foo", get(|uri: Uri| async move { uri.to_string() }));
    let app = Router::new().nest("", nested.into_service());

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = client.get("/foo").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/foo");

    let res = client.get("/foo/bar").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn nesting_handler_at_root() {
    let app = Router::new().nest("/", get(|uri: Uri| async move { uri.to_string() }));

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/");

    let res = client.get("/foo").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/foo");

    let res = client.get("/foo/bar").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/foo/bar");
}

#[tokio::test]
async fn nested_url_extractor() {
    let app = Router::new().nest(
        "/foo",
        Router::new()
            .nest(
                "/bar",
                Router::new()
                    .route("/baz", get(|uri: Uri| async move { uri.to_string() }))
                    .route(
                        "/qux",
                        get(|req: Request<Body>| async move { req.uri().to_string() }),
                    )
                    .into_service(),
            )
            .into_service(),
    );

    let client = TestClient::new(app);

    let res = client.get("/foo/bar/baz").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/baz");

    let res = client.get("/foo/bar/qux").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/qux");
}

#[tokio::test]
async fn nested_url_original_extractor() {
    let app = Router::new().nest(
        "/foo",
        Router::new()
            .nest(
                "/bar",
                Router::new()
                    .route(
                        "/baz",
                        get(|uri: extract::OriginalUri| async move { uri.0.to_string() }),
                    )
                    .into_service(),
            )
            .into_service(),
    );

    let client = TestClient::new(app);

    let res = client.get("/foo/bar/baz").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/foo/bar/baz");
}

#[tokio::test]
async fn nested_service_sees_stripped_uri() {
    let app = Router::new().nest(
        "/foo",
        Router::new()
            .nest(
                "/bar",
                Router::new()
                    .route_service(
                        "/baz",
                        service_fn(|req: Request<Body>| async move {
                            let body = boxed(Body::from(req.uri().to_string()));
                            Ok::<_, Infallible>(Response::new(body))
                        }),
                    )
                    .into_service(),
            )
            .into_service(),
    );

    let client = TestClient::new(app);

    let res = client.get("/foo/bar/baz").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/baz");
}

#[tokio::test]
async fn nest_static_file_server() {
    let app = Router::new().nest(
        "/static",
        get_service(ServeDir::new(".")).handle_error(|error| async move {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Unhandled internal error: {}", error),
            )
        }),
    );

    let client = TestClient::new(app);

    let res = client.get("/static/README.md").send().await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn nested_multiple_routes() {
    let app = Router::new()
        .nest(
            "/api",
            Router::new()
                .route("/users", get(|| async { "users" }))
                .route("/teams", get(|| async { "teams" }))
                .into_service(),
        )
        .route("/", get(|| async { "root" }));

    let client = TestClient::new(app);

    assert_eq!(client.get("/").send().await.text().await, "root");
    assert_eq!(client.get("/api/users").send().await.text().await, "users");
    assert_eq!(client.get("/api/teams").send().await.text().await, "teams");
}

#[test]
#[should_panic = "Invalid route \"/\": insertion failed due to conflict with previously registered route: /*__private__axum_nest_tail_param"]
fn nested_at_root_with_other_routes() {
    let _: Router = Router::new()
        .nest(
            "/",
            Router::new()
                .route("/users", get(|| async {}))
                .into_service(),
        )
        .route("/", get(|| async {}));
}

#[tokio::test]
async fn multiple_top_level_nests() {
    let app = Router::new()
        .nest(
            "/one",
            Router::new()
                .route("/route", get(|| async { "one" }))
                .into_service(),
        )
        .nest(
            "/two",
            Router::new()
                .route("/route", get(|| async { "two" }))
                .into_service(),
        );

    let client = TestClient::new(app);

    assert_eq!(client.get("/one/route").send().await.text().await, "one");
    assert_eq!(client.get("/two/route").send().await.text().await, "two");
}

#[tokio::test]
#[should_panic(expected = "Invalid route: nested routes cannot contain wildcards (*)")]
async fn nest_cannot_contain_wildcards() {
    Router::<_, Body>::new().nest("/one/*rest", Router::new().into_service());
}

#[tokio::test]
async fn outer_middleware_still_see_whole_url() {
    #[derive(Clone)]
    struct SetUriExtension<S>(S);

    #[derive(Clone)]
    struct Uri(http::Uri);

    impl<S, B> Service<Request<B>> for SetUriExtension<S>
    where
        S: Service<Request<B>>,
    {
        type Response = S::Response;
        type Error = S::Error;
        type Future = S::Future;

        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.0.poll_ready(cx)
        }

        fn call(&mut self, mut req: Request<B>) -> Self::Future {
            let uri = Uri(req.uri().clone());
            req.extensions_mut().insert(uri);
            self.0.call(req)
        }
    }

    async fn handler(Extension(Uri(middleware_uri)): Extension<Uri>) -> impl IntoResponse {
        middleware_uri.to_string()
    }

    let app = Router::new()
        .route("/", get(handler))
        .route("/foo", get(handler))
        .route("/foo/bar", get(handler))
        .nest(
            "/one",
            Router::new().route("/two", get(handler)).into_service(),
        )
        .fallback(handler)
        .layer(tower::layer::layer_fn(SetUriExtension));

    let client = TestClient::new(app);

    assert_eq!(client.get("/").send().await.text().await, "/");
    assert_eq!(client.get("/foo").send().await.text().await, "/foo");
    assert_eq!(client.get("/foo/bar").send().await.text().await, "/foo/bar");
    assert_eq!(
        client.get("/not-found").send().await.text().await,
        "/not-found"
    );
    assert_eq!(client.get("/one/two").send().await.text().await, "/one/two");
}

#[tokio::test]
async fn nest_at_capture() {
    let api_routes = Router::new()
        .route(
            "/:b",
            get(|Path((a, b)): Path<(String, String)>| async move { format!("a={} b={}", a, b) }),
        )
        .into_service()
        .boxed_clone();

    let app = Router::new().nest("/:a", api_routes);

    let client = TestClient::new(app);

    let res = client.get("/foo/bar").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "a=foo b=bar");
}

#[tokio::test]
async fn nest_with_and_without_trailing() {
    let app = Router::new().nest("/foo", get(|| async {}));

    let client = TestClient::new(app);

    let res = client.get("/foo").send().await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.get("/foo/").send().await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.get("/foo/bar").send().await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn doesnt_call_outer_fallback() {
    let app = Router::new()
        .nest(
            "/foo",
            Router::new().route("/", get(|| async {})).into_service(),
        )
        .fallback(|| async { (StatusCode::NOT_FOUND, "outer fallback") });

    let client = TestClient::new(app);

    let res = client.get("/foo").send().await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.get("/foo/not-found").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    // the default fallback returns an empty body
    assert_eq!(res.text().await, "");
}

#[tokio::test]
async fn nesting_with_root_inner_router() {
    let app = Router::new().nest(
        "/foo",
        Router::new()
            .route("/", get(|| async { "inner route" }))
            .into_service(),
    );

    let client = TestClient::new(app);

    // `/foo/` does match the `/foo` prefix and the remaining path is technically
    // empty, which is the same as `/` which matches `.route("/", _)`
    let res = client.get("/foo").send().await;
    assert_eq!(res.status(), StatusCode::OK);

    // `/foo/` does match the `/foo` prefix and the remaining path is `/`
    // which matches `.route("/", _)`
    let res = client.get("/foo/").send().await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn fallback_on_inner() {
    let app = Router::new()
        .nest(
            "/foo",
            Router::new()
                .route("/", get(|| async {}))
                .fallback(|| async { (StatusCode::NOT_FOUND, "inner fallback") })
                .into_service(),
        )
        .fallback(|| async { (StatusCode::NOT_FOUND, "outer fallback") });

    let client = TestClient::new(app);

    let res = client.get("/foo/not-found").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "inner fallback");
}

macro_rules! nested_route_test {
    (
        $name:ident,
        // the path we nest the inner router at
        nest = $nested_path:literal,
        // the route the inner router accepts
        route = $route_path:literal,
        // the route we expect to be able to call
        expected = $expected_path:literal $(,)?
    ) => {
        #[tokio::test]
        async fn $name() {
            let inner = Router::new().route($route_path, get(|| async {}));
            let app = Router::new().nest($nested_path, inner.into_service());
            let client = TestClient::new(app);
            let res = client.get($expected_path).send().await;
            let status = res.status();
            assert_eq!(status, StatusCode::OK, "Router");
        }
    };
}

// test cases taken from https://github.com/tokio-rs/axum/issues/714#issuecomment-1058144460
nested_route_test!(nest_1, nest = "", route = "/", expected = "/");
nested_route_test!(nest_2, nest = "", route = "/a", expected = "/a");
nested_route_test!(nest_3, nest = "", route = "/a/", expected = "/a/");
nested_route_test!(nest_4, nest = "/", route = "/", expected = "/");
nested_route_test!(nest_5, nest = "/", route = "/a", expected = "/a");
nested_route_test!(nest_6, nest = "/", route = "/a/", expected = "/a/");
nested_route_test!(nest_7, nest = "/a", route = "/", expected = "/a");
nested_route_test!(nest_8, nest = "/a", route = "/a", expected = "/a/a");
nested_route_test!(nest_9, nest = "/a", route = "/a/", expected = "/a/a/");
nested_route_test!(nest_11, nest = "/a/", route = "/", expected = "/a/");
nested_route_test!(nest_12, nest = "/a/", route = "/a", expected = "/a/a");
nested_route_test!(nest_13, nest = "/a/", route = "/a/", expected = "/a/a/");

#[tokio::test]
async fn nesting_with_different_state() {
    let inner = Router::with_state("inner".to_owned()).route(
        "/foo",
        get(|State(state): State<String>| async move { state }),
    );

    let outer = Router::with_state("outer")
        .route(
            "/foo",
            get(|State(state): State<&'static str>| async move { state }),
        )
        .nest("/nested", inner.into_service())
        .route(
            "/bar",
            get(|State(state): State<&'static str>| async move { state }),
        );

    let client = TestClient::new(outer);

    let res = client.get("/foo").send().await;
    assert_eq!(res.text().await, "outer");

    let res = client.get("/nested/foo").send().await;
    assert_eq!(res.text().await, "inner");

    let res = client.get("/bar").send().await;
    assert_eq!(res.text().await, "outer");
}
