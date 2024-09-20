# compression

This example shows how to:
- automatically decompress request bodies when necessary
- compress response bodies based on the `accept` header.

## Running

```
cargo run -p example-compression
```

## Sending compressed requests

```
curl -v -g 'http://localhost:3000/' \
    -H "Content-Type: application/json" \
    -H "Content-Encoding: gzip" \
    --compressed \
    --data-binary @data/products.json.gz
```

(Notice the `Content-Encoding: gzip` in the request, and `content-encoding: gzip` in the response.)

## Sending uncompressed requests

```
curl -v -g 'http://localhost:3000/' \
    -H "Content-Type: application/json" \
    --compressed \
    --data-binary @data/products.json
```
