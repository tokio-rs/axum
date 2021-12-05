use super::HasRoutes;
use axum::{
    body::Body,
    handler::Handler,
    http::Request,
    response::Response,
    routing::{delete, get, on, post, MethodFilter},
    Router,
};
use std::convert::Infallible;
use tower_service::Service;

/// A resource which defines a set of conventional CRUD routes.
///
/// # Example
///
/// ```rust
/// use axum::{Router, routing::get, extract::Path};
/// use axum_extra::routing::{RouterExt, Resource};
///
/// let users = Resource::named("users")
///     // Define a route for `GET /users`
///     .index(|| async {})
///     // `POST /users`
///     .create(|| async {})
///     // `GET /users/new`
///     .new(|| async {})
///     // `GET /users/:users_id`
///     .show(|Path(user_id): Path<u64>| async {})
///     // `GET /users/:users_id/edit`
///     .edit(|Path(user_id): Path<u64>| async {})
///     // `PUT or PATCH /users/:users_id`
///     .update(|Path(user_id): Path<u64>| async {})
///     // `DELETE /users/:users_id`
///     .destroy(|Path(user_id): Path<u64>| async {})
///     // Nest another router at the "member level"
///     // This defines a route for `GET /users/:users_id/tweets`
///     .nest(Router::new().route(
///         "/tweets",
///         get(|Path(user_id): Path<u64>| async {}),
///     ))
///     // Nest another router at the "collection level"
///     // This defines a route for `GET /users/featured`
///     .nest_collection(
///         Router::new().route("/featured", get(|| async {})),
///     );
///
/// let app = Router::new().with(users);
/// # let _: Router<axum::body::Body> = app;
/// ```
#[derive(Debug)]
pub struct Resource<B = Body> {
    pub(crate) name: String,
    pub(crate) router: Router<B>,
}

impl<B: Send + 'static> Resource<B> {
    /// Create a `Resource` with the given name.
    ///
    /// All routes will be nested at `/{resource_name}`.
    pub fn named(resource_name: &str) -> Self {
        Self {
            name: resource_name.to_owned(),
            router: Default::default(),
        }
    }

    /// Add a handler at `GET /{resource_name}`.
    pub fn index<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: 'static,
    {
        let path = self.index_create_path();
        self.route(&path, get(handler))
    }

    /// Add a handler at `POST /{resource_name}`.
    pub fn create<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: 'static,
    {
        let path = self.index_create_path();
        self.route(&path, post(handler))
    }

    /// Add a handler at `GET /{resource_name}/new`.
    pub fn new<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: 'static,
    {
        let path = format!("/{}/new", self.name);
        self.route(&path, get(handler))
    }

    /// Add a handler at `GET /{resource_name}/:{resource_name}_id`.
    pub fn show<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: 'static,
    {
        let path = self.show_update_destroy_path();
        self.route(&path, get(handler))
    }

    /// Add a handler at `GET /{resource_name}/:{resource_name}_id/edit`.
    pub fn edit<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: 'static,
    {
        let path = format!("/{0}/:{0}_id/edit", self.name);
        self.route(&path, get(handler))
    }

    /// Add a handler at `PUT or PATCH /resource_name/:{resource_name}_id`.
    pub fn update<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: 'static,
    {
        let path = self.show_update_destroy_path();
        self.route(&path, on(MethodFilter::PUT | MethodFilter::PATCH, handler))
    }

    /// Add a handler at `DELETE /{resource_name}/:{resource_name}_id`.
    pub fn destroy<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, B>,
        T: 'static,
    {
        let path = self.show_update_destroy_path();
        self.route(&path, delete(handler))
    }

    /// Nest another route at the "member level".
    ///
    /// The routes will be nested at `/{resource_name}/:{resource_name}_id`.
    pub fn nest<T>(mut self, svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response, Error = Infallible> + Clone + Send + 'static,
        T::Future: Send + 'static,
    {
        let path = self.show_update_destroy_path();
        self.router = self.router.nest(&path, svc);
        self
    }

    /// Nest another route at the "collection level".
    ///
    /// The routes will be nested at `/{resource_name}`.
    pub fn nest_collection<T>(mut self, svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response, Error = Infallible> + Clone + Send + 'static,
        T::Future: Send + 'static,
    {
        let path = self.index_create_path();
        self.router = self.router.nest(&path, svc);
        self
    }

    fn index_create_path(&self) -> String {
        format!("/{}", self.name)
    }

    fn show_update_destroy_path(&self) -> String {
        format!("/{0}/:{0}_id", self.name)
    }

    fn route<T>(mut self, path: &str, svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response, Error = Infallible> + Clone + Send + 'static,
        T::Future: Send + 'static,
    {
        self.router = self.router.route(path, svc);
        self
    }
}

impl<B> HasRoutes<B> for Resource<B> {
    fn routes(self) -> Router<B> {
        self.router
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use crate::routing::RouterExt;
    use axum::{extract::Path, http::Method, Router};
    use tower::ServiceExt;

    #[tokio::test]
    async fn works() {
        let users = Resource::named("users")
            .index(|| async { "users#index" })
            .create(|| async { "users#create" })
            .new(|| async { "users#new" })
            .show(|Path(id): Path<u64>| async move { format!("users#show id={}", id) })
            .edit(|Path(id): Path<u64>| async move { format!("users#edit id={}", id) })
            .update(|Path(id): Path<u64>| async move { format!("users#update id={}", id) })
            .destroy(|Path(id): Path<u64>| async move { format!("users#destroy id={}", id) })
            .nest(Router::new().route(
                "/tweets",
                get(|Path(id): Path<u64>| async move { format!("users#tweets id={}", id) }),
            ))
            .nest_collection(
                Router::new().route("/featured", get(|| async move { "users#featured" })),
            );

        let mut app = Router::new().with(users);

        assert_eq!(
            call_route(&mut app, Method::GET, "/users").await,
            "users#index"
        );

        assert_eq!(
            call_route(&mut app, Method::POST, "/users").await,
            "users#create"
        );

        assert_eq!(
            call_route(&mut app, Method::GET, "/users/new").await,
            "users#new"
        );

        assert_eq!(
            call_route(&mut app, Method::GET, "/users/1").await,
            "users#show id=1"
        );

        assert_eq!(
            call_route(&mut app, Method::GET, "/users/1/edit").await,
            "users#edit id=1"
        );

        assert_eq!(
            call_route(&mut app, Method::PATCH, "/users/1").await,
            "users#update id=1"
        );

        assert_eq!(
            call_route(&mut app, Method::PUT, "/users/1").await,
            "users#update id=1"
        );

        assert_eq!(
            call_route(&mut app, Method::DELETE, "/users/1").await,
            "users#destroy id=1"
        );

        assert_eq!(
            call_route(&mut app, Method::GET, "/users/1/tweets").await,
            "users#tweets id=1"
        );

        assert_eq!(
            call_route(&mut app, Method::GET, "/users/featured").await,
            "users#featured"
        );
    }

    async fn call_route(app: &mut Router, method: Method, uri: &str) -> String {
        let res = app
            .ready()
            .await
            .unwrap()
            .call(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = hyper::body::to_bytes(res).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }
}
