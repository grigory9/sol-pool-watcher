# Token Safety Inspector

Standalone microservice and CLI for analyzing Solana token mints for basic safety
properties. It exposes a library crate (`token_safety`), an HTTP/JSON API
(`token_safety_http`), and a small CLI (`token_safety_cli`).

```
curl -X POST localhost:8080/v1/analyze \
  -H 'Content-Type: application/json' \
  -d '{"mint":"<MINT>","probe_amount":1000,"route_supports_memo":false}'
```

```
# CLI
cargo run -p token_safety_cli -- inspect <MINT> --amount 1000
```

By default the server binds to `0.0.0.0:8080` and reads the RPC endpoint from
`RPC_URL`.
