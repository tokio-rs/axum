# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog],
and this project adheres to [Semantic Versioning].

# Unreleased

- None.

# 0.4.2 (02. December, 2022)

- **fixed:** Bug fixes for `RouterExt:{route_with_tsr, route_service_with_tsr}` ([#1608]):
  - Redirects to the correct URI if the route contains path parameters
  - Keeps query parameters when redirecting
  - Better improved error message if adding route for `/`

[#1608]: https://github.com/tokio-rs/axum/pull/1608

# 0.4.1 (29. November, 2022)

- **fixed:** Fix wrong `From` impl for `Resource` ([#1589])

[#1589]: https://github.com/tokio-rs/axum/pull/1589

# 0.4.0 (25. November, 2022)

- **added:** Add `RouterExt::route_with_tsr` for adding routes with an
  additional "trailing slash redirect" route ([#1119])
- **added:** Support chaining handlers with `HandlerCallWithExtractors::or` ([#1170])
- **added:** Add Protocol Buffer extractor and response ([#1239])
- **added:** Add `Either*` types for combining extractors and responses into a
  single type ([#1263])
- **added:** `WithRejection` extractor for customizing other extractors' rejections ([#1262])
- **added:** Add sync constructors to `CookieJar`, `PrivateCookieJar`, and
  `SignedCookieJar` so they're easier to use in custom middleware
- **changed:** For methods that accept some `S: Service`, the bounds have been
  relaxed so the return type can be any type that implements `IntoResponse` rather than being a
  literal `Response`
- **change:** axum-extra's MSRV is now 1.60 ([#1239])
- **breaking:** `Form` has a new rejection type ([#1496])
- **breaking:** `Query` has a new rejection type ([#1496])
- **breaking:** `Resource::nest` and `Resource::nest_collection` have been
  removed. You can instead convert the `Resource` into a `Router` and
  add additional routes as necessary ([#1086])
- **breaking:** `SignedCookieJar` and `PrivateCookieJar` now extracts the keys
  from the router's state, rather than extensions
- **breaking:** `Resource` has a new `S` type param which represents the state ([#1155])
- **breaking:** `RouterExt::route_with_tsr` now only accepts `MethodRouter`s ([#1155])
- **added:** `RouterExt::route_service_with_tsr` for routing to any `Service` ([#1155])

[#1086]: https://github.com/tokio-rs/axum/pull/1086
[#1119]: https://github.com/tokio-rs/axum/pull/1119
[#1155]: https://github.com/tokio-rs/axum/pull/1155
[#1170]: https://github.com/tokio-rs/axum/pull/1170
[#1214]: https://github.com/tokio-rs/axum/pull/1214
[#1239]: https://github.com/tokio-rs/axum/pull/1239
[#1262]: https://github.com/tokio-rs/axum/pull/1262
[#1263]: https://github.com/tokio-rs/axum/pull/1263
[#1496]: https://github.com/tokio-rs/axum/pull/1496

<details>
<summary>0.4.0 Pre-Releases</summary>

# 0.4.0-rc.3 (19. November, 2022)

- **breaking:** Depend axum 0.6.0-rc.5 and axum-macros 0.3.0-rc.3

# 0.4.0-rc.2 (8. November, 2022)

- **breaking:** `Form` has a new rejection type ([#1496])
- **breaking:** `Query` has a new rejection type ([#1496])

[#1496]: https://github.com/tokio-rs/axum/pull/1496

# 0.4.0-rc.1 (23. August, 2022)

- **added:** Add `RouterExt::route_with_tsr` for adding routes with an
  additional "trailing slash redirect" route ([#1119])
- **breaking:** `Resource::nest` and `Resource::nest_collection` has been
  removed. You can instead convert the `Resource` into a `Router` and
  add additional routes as necessary ([#1086])
- **changed:** For methods that accept some `S: Service`, the bounds have been
  relaxed so the response type must implement `IntoResponse` rather than being a
  literal `Response`
- **added:** Support chaining handlers with `HandlerCallWithExtractors::or` ([#1170])
- **change:** axum-extra's MSRV is now 1.60 ([#1239])
- **breaking:** `SignedCookieJar` and `PrivateCookieJar` now extracts the keys
  from the router's state, rather than extensions
- **added:** Add Protocol Buffer extractor and response ([#1239])
- **added:** Add `Either*` types for combining extractors and responses into a
  single type ([#1263])
- **added:** `WithRejection` extractor for customizing other extractors' rejections ([#1262])
- **added:** Add sync constructors to `CookieJar`, `PrivateCookieJar`, and
  `SignedCookieJar` so they're easier to use in custom middleware
- **breaking:** `Resource` has a new `S` type param which represents the state ([#1155])
- **breaking:** `RouterExt::route_with_tsr` now only accepts `MethodRouter`s ([#1155])
- **added:** `RouterExt::route_service_with_tsr` for routing to any `Service` ([#1155])

[#1086]: https://github.com/tokio-rs/axum/pull/1086
[#1119]: https://github.com/tokio-rs/axum/pull/1119
[#1155]: https://github.com/tokio-rs/axum/pull/1155
[#1170]: https://github.com/tokio-rs/axum/pull/1170
[#1214]: https://github.com/tokio-rs/axum/pull/1214
[#1239]: https://github.com/tokio-rs/axum/pull/1239
[#1262]: https://github.com/tokio-rs/axum/pull/1262
[#1263]: https://github.com/tokio-rs/axum/pull/1263

</details>

# 0.3.7 (09. August, 2022)

- **fixed:** Depend on axum 0.5.15 which contains a fix for an accidental breaking change.

# 0.3.6 (02. July, 2022)

- **fixed:** Fix feature labels missing in generated docs ([#1137])

[#1137]: https://github.com/tokio-rs/axum/pull/1137

# 0.3.5 (27. June, 2022)

- **added:** Add `JsonLines` for streaming newline delimited JSON ([#1093])
- **change:** axum-extra's MSRV is now 1.56 ([#1098])

[#1093]: https://github.com/tokio-rs/axum/pull/1093
[#1098]: https://github.com/tokio-rs/axum/pull/1098

# 0.3.4 (08. June, 2022)

- **fixed:** Use `impl IntoResponse` less in docs ([#1049])
- **added:** Add `AsyncReadBody` for creating a body from a `tokio::io::AsyncRead` ([#1072])

[#1049]: https://github.com/tokio-rs/axum/pull/1049
[#1072]: https://github.com/tokio-rs/axum/pull/1072

# 0.3.3 (18. May, 2022)

- **added:** Add `extract::Query` which supports multi-value items ([#1041])
- **added:** Support customizing rejections for `#[derive(TypedPath)]` ([#1012])

[#1041]: https://github.com/tokio-rs/axum/pull/1041
[#1012]: https://github.com/tokio-rs/axum/pull/1012

# 0.3.2 (15. May, 2022)

- **added:** Add `extract::Form` which supports multi-value items ([#1031])

[#1031]: https://github.com/tokio-rs/axum/pull/1031

# 0.3.1 (10. May, 2022)

- **fixed:** `Option` and `Result` are now supported in typed path route handler parameters ([#1001])
- **fixed:** Support wildcards in typed paths ([#1003])
- **added:** Support using a custom rejection type for `#[derive(TypedPath)]`
  instead of `PathRejection` ([#1012])

[#1001]: https://github.com/tokio-rs/axum/pull/1001
[#1003]: https://github.com/tokio-rs/axum/pull/1003
[#1012]: https://github.com/tokio-rs/axum/pull/1012

# 0.3.0 (27. April, 2022)

- **fixed:** Don't depend on axum with default features enabled ([#913])
- **breaking:** Private and signed cookies now requires enabling the
  `cookie-private` and `cookie-signed` features respectively ([#949])
- **changed:** Update to tower-http 0.3 ([#965])

[#913]: https://github.com/tokio-rs/axum/pull/913
[#949]: https://github.com/tokio-rs/axum/pull/949
[#965]: https://github.com/tokio-rs/axum/pull/965

# 0.2.1 (03. April, 2022)

- **added:** Re-export `SameSite` and `Expiration` from the `cookie` crate ([#898])
- **added:** Add `PrivateCookieJar` for managing private cookies ([#900])
- **added:** Add `SpaRouter` for routing setups commonly used for single page applications ([#904])
- **fixed:** Fix `SignedCookieJar` when using custom key types ([#899])

[#898]: https://github.com/tokio-rs/axum/pull/898
[#899]: https://github.com/tokio-rs/axum/pull/899
[#900]: https://github.com/tokio-rs/axum/pull/900
[#904]: https://github.com/tokio-rs/axum/pull/904

# 0.2.0 (31. March, 2022)

- **added:** Add `TypedPath::to_uri` for converting the path into a `Uri` ([#790])
- **added:** Extractors and responses for dealing with cookies. See `extract::cookies` for more
  details ([#816])
- **breaking:** `CachedRejection` has been removed ([#699])
- **breaking:** `<Cached<T> as FromRequest>::Rejection` is now `T::Rejection`. ([#699])
- **breaking:** `middleware::from_fn` has been remove from axum-extra and moved into the main
  axum crate ([#719])
- **breaking:** `HasRoutes` has been removed. `Router::merge` now accepts `Into<Router>` ([#819])
- **breaking:** `RouterExt::with` method has been removed. Use `Router::merge` instead. It works
  identically ([#819])

[#699]: https://github.com/tokio-rs/axum/pull/699
[#719]: https://github.com/tokio-rs/axum/pull/719
[#790]: https://github.com/tokio-rs/axum/pull/790
[#816]: https://github.com/tokio-rs/axum/pull/816
[#819]: https://github.com/tokio-rs/axum/pull/819

# 0.1.5 (1. March, 2022)

- **added:** Add `TypedPath::to_uri` for converting the path into a `Uri` ([#790])

[#790]: https://github.com/tokio-rs/axum/pull/790

# 0.1.4 (22. February, 2022)

- **fix:** Depend on the right versions of axum and axum-macros ([#782])

[#782]: https://github.com/tokio-rs/axum/pull/782

# 0.1.3 (22. February, 2022)

- **added:** Add type safe routing. See `axum_extra::routing::typed` for more details ([#756])
- **fix:** Depend on tower with `default_features = false` ([#666])
- **change:** `middleware::from_fn` has been deprecated and moved into the main
  axum crate ([#719])

[#666]: https://github.com/tokio-rs/axum/pull/666
[#719]: https://github.com/tokio-rs/axum/pull/719
[#756]: https://github.com/tokio-rs/axum/pull/756

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
