use axum::{
    body::{Body, HttpBody},
    Router,
};
use std::{
    any::type_name,
    fmt,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::Arc,
};
use tower_http::services::{ServeDir, ServeFile};

/// Router for single page applications.
///
/// `SpaRouter` gives a routing setup commonly used for single page applications.
///
/// # Example
///
/// ```
/// use axum_extra::routing::SpaRouter;
/// use axum::{Router, routing::get};
///
/// let spa = SpaRouter::new("/assets", "dist");
///
/// let app = Router::new()
///     // `SpaRouter` implements `Into<Router>` so it works with `merge`
///     .merge(spa)
///     // we can still add other routes
///     .route("/api/foo", get(api_foo));
/// # let _: Router = app;
///
/// async fn api_foo() {}
/// ```
///
/// With this setup we get this behavior:
///
/// - `GET /` will serve `index.html`
/// - `GET /assets/app.js` will serve `dist/app.js` assuming that file exists
/// - `GET /assets/doesnt_exist` will respond with `404 Not Found` assuming no
///   such file exists
/// - `GET /some/other/path` will serve `index.html` since there isn't another
///   route for it
/// - `GET /api/foo` will serve the `api_foo` handler function
pub struct SpaRouter<S = (), B = Body> {
    paths: Arc<Paths>,
    _marker: PhantomData<fn() -> (S, B)>,
}

#[derive(Debug)]
struct Paths {
    assets_path: String,
    assets_dir: PathBuf,
    index_file: PathBuf,
}

impl<S, B> SpaRouter<S, B> {
    /// Create a new `SpaRouter`.
    ///
    /// Assets will be served at `GET /{serve_assets_at}` from the directory at `assets_dir`.
    ///
    /// The index file defaults to `assets_dir.join("index.html")`.
    pub fn new<P>(serve_assets_at: &str, assets_dir: P) -> Self
    where
        P: AsRef<Path>,
    {
        let path = assets_dir.as_ref();
        Self {
            paths: Arc::new(Paths {
                assets_path: serve_assets_at.to_owned(),
                assets_dir: path.to_owned(),
                index_file: path.join("index.html"),
            }),
            _marker: PhantomData,
        }
    }
}

impl<S, B> SpaRouter<S, B> {
    /// Set the path to the index file.
    ///
    /// `path` must be relative to `assets_dir` passed to [`SpaRouter::new`].
    ///
    /// # Example
    ///
    /// ```
    /// use axum_extra::routing::SpaRouter;
    /// use axum::Router;
    ///
    /// let spa = SpaRouter::new("/assets", "dist")
    ///     .index_file("another_file.html");
    ///
    /// let app = Router::new().merge(spa);
    /// # let _: Router = app;
    /// ```
    pub fn index_file<P>(mut self, path: P) -> Self
    where
        P: AsRef<Path>,
    {
        self.paths = Arc::new(Paths {
            assets_path: self.paths.assets_path.clone(),
            assets_dir: self.paths.assets_dir.clone(),
            index_file: self.paths.assets_dir.join(path),
        });
        self
    }
}

impl<S, B> From<SpaRouter<S, B>> for Router<S, B>
where
    B: HttpBody + Send + 'static,
    S: Clone + Send + Sync + 'static,
{
    fn from(spa: SpaRouter<S, B>) -> Router<S, B> {
        let assets_service = ServeDir::new(&spa.paths.assets_dir);
        Router::new()
            .nest_service(&spa.paths.assets_path, assets_service)
            .fallback_service(ServeFile::new(&spa.paths.index_file))
    }
}

impl<B, T> fmt::Debug for SpaRouter<B, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { paths, _marker } = self;

        f.debug_struct("SpaRouter")
            .field("paths", &paths)
            .field("request_body_type", &format_args!("{}", type_name::<B>()))
            .field(
                "extractor_input_type",
                &format_args!("{}", type_name::<T>()),
            )
            .finish()
    }
}

impl<B, T> Clone for SpaRouter<B, T> {
    fn clone(&self) -> Self {
        Self {
            paths: self.paths.clone(),
            _marker: self._marker,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::routing::get;
    use http::StatusCode;

    #[tokio::test]
    async fn basic() {
        let app = Router::new()
            .route("/foo", get(|| async { "GET /foo" }))
            .merge(SpaRouter::new("/assets", "test_files"));
        let client = TestClient::new(app);

        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "<h1>Hello, World!</h1>\n");

        let res = client.get("/some/random/path").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "<h1>Hello, World!</h1>\n");

        let res = client.get("/assets/script.js").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "console.log('hi')\n");

        let res = client.get("/foo").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "GET /foo");

        let res = client.get("/assets/doesnt_exist").send().await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn setting_index_file() {
        let app =
            Router::new().merge(SpaRouter::new("/assets", "test_files").index_file("index_2.html"));
        let client = TestClient::new(app);

        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "<strong>Hello, World!</strong>\n");

        let res = client.get("/some/random/path").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "<strong>Hello, World!</strong>\n");
    }

    #[allow(dead_code)]
    fn works_with_router_with_state() {
        let _: Router = Router::new()
            .merge(SpaRouter::new("/assets", "test_files"))
            .route("/", get(|_: axum::extract::State<String>| async {}))
            .with_state(String::new());
    }
}
