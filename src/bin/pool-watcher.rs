use clap::Parser;
use pool_watcher::token::TokenSafetyProvider;
use pool_watcher::{PoolBus, PoolWatcher, PoolWatcherConfig};
use solana_client::rpc_client::RpcClient;
use std::{fs, path::PathBuf, sync::Arc};

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// Path to TOML configuration file
    #[arg(short, long, default_value = "pool-watcher.toml")]
    config: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let cfg: PoolWatcherConfig = match fs::read_to_string(&args.config) {
        Ok(data) => toml::from_str(&data)?,
        Err(_) => PoolWatcherConfig::default(),
    };

    let bus = Arc::new(PoolBus::new(1024));
    let rpc = RpcClient::new(cfg.rpc_url.clone());
    let token = Arc::new(TokenSafetyProvider::new(rpc));
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
