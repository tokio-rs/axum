use axum::{
    body::{Body, HttpBody},
    error_handling::HandleError,
    response::Response,
    routing::{get_service, Route},
    Router,
};
use http::{Request, StatusCode};
use std::{
    any::type_name,
    convert::Infallible,
    fmt,
    future::{ready, Ready},
    io,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::Arc,
};
use tower_http::services::{ServeDir, ServeFile};
use tower_service::Service;

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
/// # let _: Router<axum::body::Body> = app;
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
pub struct SpaRouter<B = Body, T = (), F = fn(io::Error) -> Ready<StatusCode>> {
    paths: Arc<Paths>,
    handle_error: F,
    _marker: PhantomData<fn() -> (B, T)>,
}

#[derive(Debug)]
struct Paths {
    assets_path: String,
    assets_dir: PathBuf,
    index_file: PathBuf,
}

impl<B> SpaRouter<B, (), fn(io::Error) -> Ready<StatusCode>> {
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
            handle_error: |_| ready(StatusCode::INTERNAL_SERVER_ERROR),
            _marker: PhantomData,
        }
    }
}

impl<B, T, F> SpaRouter<B, T, F> {
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
    /// # let _: Router<axum::body::Body> = app;
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

    /// Change the function used to handle unknown IO errors.
    ///
    /// `SpaRouter` automatically maps missing files and permission denied to
    /// `404 Not Found`. The callback given here will be used for other IO errors.
    ///
    /// See [`axum::error_handling::HandleErrorLayer`] for more details.
    ///
    /// # Example
    ///
    /// ```
    /// use std::io;
    /// use axum_extra::routing::SpaRouter;
    /// use axum::{Router, http::{Method, Uri}};
    ///
    /// let spa = SpaRouter::new("/assets", "dist").handle_error(handle_error);
    ///
    /// async fn handle_error(method: Method, uri: Uri, err: io::Error) -> String {
    ///     format!("{} {} failed with {}", method, uri, err)
    /// }
    ///
    /// let app = Router::new().merge(spa);
    /// # let _: Router<axum::body::Body> = app;
    /// ```
    pub fn handle_error<T2, F2>(self, f: F2) -> SpaRouter<B, T2, F2> {
        SpaRouter {
            paths: self.paths,
            handle_error: f,
            _marker: PhantomData,
        }
    }
}

impl<B, F, T> From<SpaRouter<B, T, F>> for Router<B>
where
    F: Clone + Send + 'static,
    HandleError<Route<B, io::Error>, F, T>:
        Service<Request<B>, Response = Response, Error = Infallible>,
    <HandleError<Route<B, io::Error>, F, T> as Service<Request<B>>>::Future: Send,
    B: HttpBody + Send + 'static,
    T: 'static,
{
    fn from(spa: SpaRouter<B, T, F>) -> Self {
        let assets_service = get_service(ServeDir::new(&spa.paths.assets_dir))
            .handle_error(spa.handle_error.clone());

        Router::new()
            .nest(&spa.paths.assets_path, assets_service)
            .fallback(
                get_service(ServeFile::new(&spa.paths.index_file)).handle_error(spa.handle_error),
            )
    }
}

impl<B, T, F> fmt::Debug for SpaRouter<B, T, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            paths,
            handle_error: _,
            _marker,
        } = self;

        f.debug_struct("SpaRouter")
            .field("paths", &paths)
            .field("handle_error", &format_args!("{}", type_name::<F>()))
            .field("request_body_type", &format_args!("{}", type_name::<B>()))
            .field(
                "extractor_input_type",
                &format_args!("{}", type_name::<T>()),
            )
            .finish()
    }
}

impl<B, T, F> Clone for SpaRouter<B, T, F>
where
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            paths: self.paths.clone(),
            handle_error: self.handle_error.clone(),
            _marker: self._marker,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{
        http::{Method, Uri},
        routing::get,
    };

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

    // this should just compile
    #[allow(dead_code)]
    fn setting_error_handler() {
        async fn handle_error(method: Method, uri: Uri, err: io::Error) -> (StatusCode, String) {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("{} {} failed. Error: {}", method, uri, err),
            )
        }

        let spa = SpaRouter::new("/assets", "test_files").handle_error(handle_error);

        Router::<Body>::new().merge(spa);
    }
}
