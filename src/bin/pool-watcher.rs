use clap::Parser;
use pool_watcher::service::TelegramConfig;
use pool_watcher::token::TokenSafetyProvider;
use pool_watcher::{PoolBus, PoolWatcher, PoolWatcherConfig};
use reqwest::blocking::Client;
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
    let telegram_cfg = cfg.telegram.clone();
    let watcher = PoolWatcher::new(cfg, bus.clone(), token);
    watcher.spawn();

    let mut rx = bus.subscribe();
    let client = telegram_cfg.as_ref().map(|_| Client::new());
    loop {
        match rx.blocking_recv() {
            Ok(pool_watcher::PoolEvent::AccountNew { info, .. }) => {
                println!("{:?}", info);
                if let (Some(cfg), Some(client)) = (&telegram_cfg, &client) {
                    if !info.is_token2022_base && !info.is_token2022_quote {
                        let base = info
                            .base_mint
                            .map(|m| m.to_string())
                            .unwrap_or_default();
                        let quote = info
                            .quote_mint
                            .map(|m| m.to_string())
                            .unwrap_or_default();
                        let text = format!("New pool: {base}/{quote}");
                        if let Err(e) = send_telegram(client, cfg, &text) {
                            eprintln!("telegram send failed: {e:?}");
                        }
                    }
                }
            }
            Ok(ev) => println!("{:?}", ev),
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(_) => break,
        }
    }
    Ok(())
}

fn send_telegram(client: &Client, cfg: &TelegramConfig, text: &str) -> anyhow::Result<()> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", cfg.bot_token);
    client
        .post(url)
        .form(&[("chat_id", cfg.chat_id.as_str()), ("text", text)])
        .send()?
        .error_for_status()?;
    Ok(())
}
