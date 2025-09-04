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
cargo run --release --bin pool-watcher -- -c pool-watcher.toml
```

## Subscribing to `PoolBus` events

`PoolWatcher` broadcasts [`PoolEvent`](src/types.rs) messages over a [`PoolBus`](src/bus.rs) so that consumers can listen for updates:

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

For deeper inspection of token metadata or supply, see the [`token-safety-inspector`](token-safety-inspector) workspace.

## Telegram notifications

Optionally configure a `[telegram]` section in `pool-watcher.toml` with your bot token and chat id. When enabled, the `pool-watcher`
binary sends a message whenever it discovers a new pool whose tokens are standard SPL mints (not token2022).

