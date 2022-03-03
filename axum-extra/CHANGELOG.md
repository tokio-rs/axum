# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog],
and this project adheres to [Semantic Versioning].

# Unreleased

- **added:** Add `TypedPath::to_uri` for converting the path into a `Uri` ([#790])
- **added:** Add type safe routing. See `axum_extra::routing::typed` for more details ([#756])
- **added:** Extractors and responses for dealing with cookies. See `extract::cookies` for more
  details ([#816])
- **breaking:** `CachedRejection` has been removed ([#699])
- **breaking:** `<Cached<T> as FromRequest>::Rejection` is now `T::Rejection`. ([#699])
- **breaking:** `middleware::from_fn` has been moved into the main axum crate ([#719])
- **breaking:** `HasRoutes` has been removed. `Router::merge` now accepts `Into<Router>` ([#819])
- **breaking:** `RouterExt::with` method has been removed. Use `Router::merge` instead. It works
  identically ([#819])

[#666]: https://github.com/tokio-rs/axum/pull/666
[#699]: https://github.com/tokio-rs/axum/pull/699
[#719]: https://github.com/tokio-rs/axum/pull/719
[#756]: https://github.com/tokio-rs/axum/pull/756
[#790]: https://github.com/tokio-rs/axum/pull/790
[#816]: https://github.com/tokio-rs/axum/pull/816
[#819]: https://github.com/tokio-rs/axum/pull/819

# 0.1.2 (13. January, 2022)

- **fix:** Depend on tower with `default_features = false` ([#666])

# 0.1.1 (27. December, 2021)

- Add `middleware::from_fn` for creating middleware from async functions ([#656])
- Add support for returning pretty JSON response in `response::ErasedJson` ([#662])

[#656]: https://github.com/tokio-rs/axum/pull/656
[#662]: https://github.com/tokio-rs/axum/pull/662

# 0.1.0 (02. December, 2021)

- Initial release.

[Keep a Changelog]: https://keepachangelog.com/en/1.0.0/
[Semantic Versioning]: https://semver.org/spec/v2.0.0.html
