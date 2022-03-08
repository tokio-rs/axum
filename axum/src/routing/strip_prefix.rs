use crate::util::IteratorExt;
use http::{Request, Uri};
use std::{
    borrow::Cow,
    sync::Arc,
    task::{Context, Poll},
};
use tower_service::Service;

#[derive(Clone)]
pub(super) struct StripPrefix<S> {
    inner: S,
    prefix: Arc<str>,
}

impl<S> StripPrefix<S> {
    pub(super) fn new(inner: S, prefix: &str) -> Self {
        Self {
            inner,
            prefix: prefix.into(),
        }
    }
}

impl<S, B> Service<Request<B>> for StripPrefix<S>
where
    S: Service<Request<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        let new_uri = strip_prefix(req.uri(), &self.prefix);
        *req.uri_mut() = new_uri;
        self.inner.call(req)
    }
}

#[allow(unused_must_use)]
fn strip_prefix(uri: &Uri, prefix: &str) -> Uri {
    let path_and_query = uri.path_and_query().map(|path_and_query| {
        let mut matching_prefix_length = Some(0);

        for (path_segment, prefix_segment) in segments(path_and_query.path()).zip(segments(prefix))
        {
            // add the `/`
            *matching_prefix_length.as_mut().unwrap() += 1;

            // dbg!((&path_segment, &prefix_segment));

            match (path_segment, prefix_segment) {
                (Some(path_segment), Some(prefix_segment)) => {
                    if prefix_segment.starts_with(':') || path_segment == prefix_segment {
                        *matching_prefix_length.as_mut().unwrap() += path_segment.len();
                    } else {
                        matching_prefix_length = None;
                        break;
                    }
                }
                // the prefix had more segments than the path
                // it cannot match
                (None, Some(_)) => {
                    matching_prefix_length = None;
                    break;
                }
                // the path had more segments than the prefix
                // the prefix might still match
                (Some(_), None) => {
                    break;
                }
                // path and prefix had same number of segments
                (None, None) => {
                    break;
                }
            }
        }

        let prefix_with_interpolations = if let Some(idx) = matching_prefix_length {
            dbg!(&idx);
            dbg!(&uri.path());
            let (prefix, _) = uri.path().split_at(idx - 1);
            prefix
        } else {
            return path_and_query.clone();
        };

        let path_after_prefix = path_and_query
            .path()
            .strip_prefix(&prefix_with_interpolations)
            .unwrap_or_else(|| path_and_query.path());

        let new_path = if path_after_prefix.starts_with('/') {
            Cow::Borrowed(path_after_prefix)
        } else {
            Cow::Owned(format!("/{}", path_after_prefix))
        };

        if let Some(query) = path_and_query.query() {
            format!("{}?{}", new_path, query).parse().unwrap()
        } else {
            new_path.parse().unwrap()
        }
    });

    let mut parts = http::uri::Parts::default();
    parts.scheme = uri.scheme().cloned();
    parts.authority = uri.authority().cloned();
    parts.path_and_query = path_and_query;

    Uri::from_parts(parts).unwrap()
}

fn segments(s: &str) -> impl Iterator<Item = Option<&str>> {
    assert!(
        s.starts_with('/'),
        "path didn't start with '/'. axum should have caught this higher up."
    );

    s.split('/')
        // skip one because paths always start with `/` so `/a/b` would become ["", "a", "b"]
        // otherwise
        .skip(1)
        .map(Some)
        .chain(std::iter::repeat_with(|| None))
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    macro_rules! test {
        (
            $name:ident,
            uri = $uri:literal,
            prefix = $prefix:literal,
            expected = $expected:literal $(,)?
        ) => {
            #[test]
            fn $name() {
                let uri = $uri.parse().unwrap();
                assert_eq!(
                    strip_prefix(&uri, $prefix),
                    $expected,
                    "without query params"
                );

                let uri = concat!($uri, "?foo=bar").parse().unwrap();
                assert_eq!(
                    strip_prefix(&uri, $prefix),
                    concat!($expected, "?foo=bar"),
                    "with query params"
                );
            }
        };
    }

    test!(empty, uri = "/", prefix = "/", expected = "/");

    test!(single_segment, uri = "/a", prefix = "/a", expected = "/");
    test!(
        single_segment_root_uri,
        uri = "/",
        prefix = "/a",
        expected = "/"
    );
    test!(
        single_segment_root_prefix,
        uri = "/a",
        prefix = "/",
        expected = "/a"
    );
    test!(
        single_segment_missing,
        uri = "/a",
        prefix = "/b",
        expected = "/a"
    );

    test!(
        single_segment_trailing_slash,
        uri = "/a/",
        prefix = "/a/",
        expected = "/"
    );
    // TODO(david): write integration test for this, is this behavior correct?
    test!(
        single_segment_trailing_slash_2,
        uri = "/a",
        prefix = "/a/",
        expected = "/a"
    );
    test!(
        single_segment_trailing_slash_3,
        uri = "/a/",
        prefix = "/a",
        expected = "/"
    );

    test!(multi_segment, uri = "/a/b", prefix = "/a", expected = "/b");
    test!(
        multi_segment_2,
        uri = "/b/a",
        prefix = "/a",
        expected = "/b/a"
    );
    test!(
        multi_segment_3,
        uri = "/a",
        prefix = "/a/b",
        expected = "/a"
    );
    test!(
        multi_segment_4,
        uri = "/a/b",
        prefix = "/b",
        expected = "/a/b"
    );

    test!(
        multi_segment_trailing_slash,
        uri = "/a/b/",
        prefix = "/a/b/",
        expected = "/"
    );
    // TODO(david): write integration test for this, is this behavior correct?
    test!(
        multi_segment_trailing_slash_2,
        uri = "/a/b",
        prefix = "/a/b/",
        expected = "/a/b"
    );
    test!(
        multi_segment_trailing_slash_3,
        uri = "/a/b/",
        prefix = "/a/b",
        expected = "/"
    );

    test!(param_0, uri = "/", prefix = "/:param", expected = "/");
    test!(param_1, uri = "/a", prefix = "/:param", expected = "/");
    test!(param_2, uri = "/a/b", prefix = "/:param", expected = "/b");
    test!(param_3, uri = "/b/a", prefix = "/:param", expected = "/a");
    test!(param_4, uri = "/a/b", prefix = "/a/:param", expected = "/");
    test!(
        param_5,
        uri = "/b/a",
        prefix = "/a/:param",
        expected = "/b/a"
    );
    test!(
        param_6,
        uri = "/a/b",
        prefix = "/:param/a",
        expected = "/a/b"
    );
    test!(param_7, uri = "/b/a", prefix = "/:param/a", expected = "/");
    test!(
        param_8,
        uri = "/a/b/c",
        prefix = "/a/:param/c",
        expected = "/"
    );
    test!(
        param_9,
        uri = "/c/b/a",
        prefix = "/a/:param/c",
        expected = "/c/b/a"
    );
    test!(param_10, uri = "/a/", prefix = "/:param", expected = "/");
    test!(param_11, uri = "/a", prefix = "/:param/", expected = "/a");
    test!(param_12, uri = "/a/", prefix = "/:param/", expected = "/");

    // TODO(david): quickcheck tests that we don't panic
}
