This example explores 3 different ways you can create custom rejections for
already existing extractors

- [`with_rejection`](src/with_rejection.rs): Uses
  `axum_extra::extract::WithRejection` to transform one rejection into another
- [`derive_from_request`](src/derive_from_request.rs): Uses
  `axum_macros::FromRequest` to wrap another extractor and customize the
  rejection
- [`custom_extractor`](src/custom_extractor.rs): Manual implementation of
  `FromRequest` that wraps another extractor

Run with

```sh
cd examples && cargo run -p example-customize-extractor-error
```
