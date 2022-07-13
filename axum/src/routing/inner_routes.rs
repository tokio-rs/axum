use std::{collections::HashMap, fmt, sync::Arc};

/// The low level router used to route routes added with `Router::route` as well as custom
/// fallbacks
pub(super) struct InnerRoutes<T> {
    route_id_to_endpoint: HashMap<RouteId, T>,
    node: Arc<Node>,
}

impl<T> InnerRoutes<T> {
    pub(super) fn try_insert<'path>(
        &mut self,
        path: &'path str,
        route: T,
    ) -> Result<(), InsertError<'path, T>> {
        let id = RouteId::next();

        let mut node =
            Arc::try_unwrap(Arc::clone(&self.node)).unwrap_or_else(|node| (*node).clone());

        if let Err(err) = node.insert(path, id) {
            return Err(InsertError { err, path, route });
        }

        self.node = Arc::new(node);

        self.route_id_to_endpoint.insert(id, route);

        Ok(())
    }

    pub(super) fn overwrite(&mut self, path: &str, route: T) {
        match self.try_insert(path, route) {
            Ok(_) => {}
            Err(err) => {
                let id = self.node.path_to_route_id[path];
                self.route_id_to_endpoint.insert(id, err.route);
            }
        }
    }

    pub(super) fn get_route(&self, path: &str) -> Option<&T> {
        let id = self.node.path_to_route_id.get(path)?;
        Some(&self.route_id_to_endpoint[id])
    }

    pub(super) fn into_iter(self) -> impl Iterator<Item = (Arc<str>, T)> {
        self.route_id_to_endpoint
            .into_iter()
            .map(move |(route_id, route)| {
                let path = &self.node.route_id_to_path[&route_id];
                let path = Arc::clone(path);
                (path, route)
            })
    }

    pub(super) fn at<'router, 'path>(
        &'router self,
        path: &'path str,
    ) -> Option<Match<'router, 'path, T>> {
        let matchit::Match { value: id, params } = self.node.at(path).ok()?;

        #[cfg(feature = "matched-path")]
        let matched_path = self.node.route_id_to_path.get(id).unwrap();

        let route = self.route_id_to_endpoint.get(id).unwrap();

        Some(Match {
            params,
            #[cfg(feature = "matched-path")]
            matched_path,
            route,
        })
    }

    pub(super) fn map_routes<F, K>(self, f: F) -> InnerRoutes<K>
    where
        F: Fn(T) -> K,
    {
        let route_id_to_endpoint = self
            .route_id_to_endpoint
            .into_iter()
            .map(|(id, route)| (id, f(route)))
            .collect();

        InnerRoutes {
            route_id_to_endpoint,
            node: self.node,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct RouteId(u32);

impl RouteId {
    fn next() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        // `AtomicU64` isn't supported on all platforms
        static ID: AtomicU32 = AtomicU32::new(0);
        let id = ID.fetch_add(1, Ordering::Relaxed);
        if id == u32::MAX {
            panic!("Over `u32::MAX` routes created. If you need this, please file an issue.");
        }
        Self(id)
    }
}

impl fmt::Debug for RouteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RouteId({})", self.0)
    }
}

#[derive(Debug)]
pub(super) struct InsertError<'a, T> {
    err: matchit::InsertError,
    path: &'a str,
    route: T,
}

impl<'a, T> fmt::Display for InsertError<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid route {:?}: {}", self.path, self.err)
    }
}

pub(super) struct Match<'router, 'path, T> {
    pub(super) params: matchit::Params<'router, 'path>,
    #[cfg(feature = "matched-path")]
    pub(super) matched_path: &'router Arc<str>,
    pub(super) route: &'router T,
}

impl<T> Default for InnerRoutes<T> {
    fn default() -> Self {
        Self {
            route_id_to_endpoint: Default::default(),
            node: Default::default(),
        }
    }
}

impl<T> Clone for InnerRoutes<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            route_id_to_endpoint: self.route_id_to_endpoint.clone(),
            node: Arc::clone(&self.node),
        }
    }
}

impl<T> fmt::Debug for InnerRoutes<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InnerRoutes")
            .field("route_id_to_endpoint", &self.route_id_to_endpoint)
            .field("node", &self.node)
            .finish()
    }
}

/// Wrapper around `matchit::Router` that supports merging
#[derive(Clone, Default)]
struct Node {
    inner: matchit::Router<RouteId>,
    route_id_to_path: HashMap<RouteId, Arc<str>>,
    path_to_route_id: HashMap<Arc<str>, RouteId>,
}

impl Node {
    fn insert(
        &mut self,
        path: impl Into<String>,
        val: RouteId,
    ) -> Result<(), matchit::InsertError> {
        let path = path.into();

        self.inner.insert(&path, val)?;

        let shared_path: Arc<str> = path.into();
        self.route_id_to_path.insert(val, shared_path.clone());
        self.path_to_route_id.insert(shared_path, val);

        Ok(())
    }

    fn at<'n, 'p>(
        &'n self,
        path: &'p str,
    ) -> Result<matchit::Match<'n, 'p, &'n RouteId>, matchit::MatchError> {
        self.inner.at(path)
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("route_id_to_path", &self.route_id_to_path)
            .field("path_to_route_id", &self.path_to_route_id)
            .finish()
    }
}
