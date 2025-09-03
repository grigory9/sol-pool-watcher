use std::{fs, path::PathBuf, sync::Arc};
use clap::Parser;
use pool_watcher::{PoolBus, PoolWatcher, PoolWatcherConfig, TokenIntrospectionProvider};
use solana_sdk::pubkey::Pubkey;

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// Path to TOML configuration file
    #[arg(short, long, default_value = "pool-watcher.toml")]
    config: PathBuf,
}

struct NoopTokenProvider;

impl TokenIntrospectionProvider for NoopTokenProvider {
    fn is_token2022(&self, _mint: &Pubkey) -> anyhow::Result<bool> {
        Ok(false)
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let cfg: PoolWatcherConfig = match fs::read_to_string(&args.config) {
        Ok(data) => toml::from_str(&data)?,
        Err(_) => PoolWatcherConfig::default(),
    };

    let bus = Arc::new(PoolBus::new(1024));
    let token = Arc::new(NoopTokenProvider);
    let watcher = PoolWatcher::new(cfg, bus.clone(), token);
    watcher.spawn();

    let mut rx = bus.subscribe();
    loop {
        match rx.blocking_recv() {
            Ok(ev) => println!("{:?}", ev),
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(_) => break,
        }
    }
    Ok(())
}
