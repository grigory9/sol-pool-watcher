# Sol Pool Watcher

Monitors Solana liquidity pool programs and publishes events about new or changing pools. Use it to watch for fresh liquidity and perform custom reactions such as automated sniping.

## Requirements
- Rust 1.70+

## Running

Configure RPC endpoints and the programs you want to watch in `pool-watcher.toml`:

```toml
rpc_url = "https://api.mainnet-beta.solana.com"
ws_url = "wss://api.mainnet-beta.solana.com"
periodic_resync_min = 30

[[programs]]
kind = "OrcaWhirlpools"
id = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"
```

Start the watcher using Cargo:

```bash
cargo run --release -p pool_watcher --bin pool-watcher -- -c pool-watcher.toml
```

## Subscribing to `PoolBus` events

`PoolWatcher` broadcasts [`PoolEvent`](crates/pool_watcher/src/types.rs) messages over a [`PoolBus`](crates/pool_watcher/src/bus.rs) so that consumers can listen for updates:

```rust
use std::sync::Arc;
use pool_watcher::{PoolBus, PoolEvent, PoolWatcher, PoolWatcherConfig};

let cfg = PoolWatcherConfig::default();
let bus = Arc::new(PoolBus::new(1024));
let watcher = PoolWatcher::new(cfg, bus.clone(), /* token provider */);
watcher.spawn();

let mut rx = bus.subscribe();
tokio::spawn(async move {
    while let Ok(PoolEvent::AccountNew { info, .. }) = rx.recv().await {
        println!("new pool: {:?}", info);
        // insert sniping logic here
    }
});
```

## Token checks

For deeper inspection of token metadata or supply, see the [`token-safety-inspector`](crates/token-safety-inspector) workspace.


## Telegram publishing and token analysis

This repository now includes reusable crates for token decoding (`token_decode`),
Telegram publishing (`tg_publisher`), quick liquidity metrics (`liq_metrics`)
and hype scoring (`hype_score`). They share common structs in `common_types`.

To publish pool alerts to Telegram, set the following environment variables:

```
TG_BOT_TOKEN=123456:ABCDEF
TG_CHANNEL_ID=@channel_name
TG_SEND_JSON_ATTACHMENT=true
```

`tg_publisher` automatically escapes MarkdownV2 and can attach the structured
`PoolTokenBundle` as a JSON document.

The JSON schema for a `PoolTokenBundle` alert is available in
`docs/pool_token_bundle.schema.json`.

## arb-notify orchestrator

The `arb-notify` binary wires together the watcher, token analysis, liquidity
metrics, hype scoring and Telegram publishing into a single runtime. Alerts are
persisted as JSONL files and published to Telegram.

### Environment

```
RPC_URL=https://api.mainnet-beta.solana.com
OUT_DIR=./outbox
TG_BOT_TOKEN=123:ABC
TG_CHANNEL_ID=@mychannel
QUOTE_MINTS=So11111111111111111111111111111111111111112,Es9vMFrzaCERFqqY5wNedGqc8ZG9wirtmHG2d ...
PROBE_AMOUNT=1000000
```

### Run

```
cargo run --release -p pool_watcher --bin arb-notify
```

`arb-notify` will create files such as `outbox/alerts_enriched-2024-01-01.jsonl`
containing one JSON object per line:

```
{"bundle":{...},"liq":{...},"hype":null}
```

Errors from Telegram publishing are written to `outbox/errors-YYYY-MM-DD.jsonl`.

![telegram](docs/telegram_sample.png)
