# Example OAuth implementation

1. Start [oauth2-test-server](https://github.com/rust-mcp-stack/oauth2-test-server).
   ```console
   oauth2-test-server
   ```
1. Register a client.
   ```console
   REGISTRATION="$(curl -s http://127.0.0.1:8090/register --json @registration.json)"
   ```
1. Run the example.
   ```console
   CLIENT_ID="$(echo "$REGISTRATION" | jq -r '.client_id')" \
   CLIENT_SECRET="$(echo "$REGISTRATION" | jq -r '.client_secret')" \
   cargo run --package example-oauth
   ```
