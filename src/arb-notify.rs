use std::{
    fs,
    num::NonZeroUsize,
    path::PathBuf,
    str::FromStr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use common_types::{EnrichedPoolAlert, PoolTokenBundle, TokenSafetyReport};
use file_sink::{FileSink, FileSinkCfg};
use hype_score::{HypeAggregator, HypeConfig, PoolLogEvent};
use liq_metrics::{compute_quick, PoolInput};
use lru::LruCache;
use pool_watcher::{
    token::TokenSafetyProvider, types::PoolEvent, PoolBus, PoolWatcher, PoolWatcherConfig,
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use serde::Deserialize;
use tg_publisher::{TgConfig, TgPublisher};
use token_decode::{analyze_mint, policy::Policy};
use tokio::sync::Mutex;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = Config::from_file("arb-config.toml");
    let rpc = Arc::new(RpcClient::new(cfg.rpc_url.clone()));
    let tg = TgPublisher::new(cfg.tg.clone())?;
    let sink = FileSink::new(FileSinkCfg {
        dir: cfg.out_dir.clone(),
        rotate_daily: true,
    })
    .await?;
    let hype = Arc::new(HypeAggregator::new(cfg.hype_cfg.clone()));

    let bus = Arc::new(PoolBus::new(2048));
    let watcher_rpc = RpcClient::new(cfg.rpc_url.clone());
    let token_provider = Arc::new(TokenSafetyProvider::new(watcher_rpc));
    PoolWatcher::new(default_watcher_cfg(&cfg), bus.clone(), token_provider).spawn();

    spawn_logs_ingestor(bus.clone(), hype.clone());
    spawn_pool_pipeline(
        bus.clone(),
        rpc.clone(),
        tg.clone(),
        sink.clone(),
        hype.clone(),
        cfg,
    )
    .await;

    futures::future::pending::<()>().await;
    Ok(())
}

fn default_watcher_cfg(cfg: &Config) -> PoolWatcherConfig {
    let mut c = PoolWatcherConfig::default();
    c.rpc_url = cfg.rpc_url.clone();
    c.ws_url = cfg.ws_url.clone();
    c
}

fn current_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[derive(Clone)]
struct Config {
    rpc_url: String,
    ws_url: String,
    out_dir: PathBuf,
    quote_mints: Vec<Pubkey>,
    probe_amount: u64,
    policy: Policy,
    hype_cfg: HypeConfig,
    tg: TgConfig,
}

impl Config {
    fn from_file(path: &str) -> Self {
        let data = fs::read_to_string(path).expect("config read failed");
        let RawConfig {
            rpc_url,
            ws_url,
            out_dir,
            quote_mints,
            probe_amount,
            policy,
            hype,
            telegram,
        } = toml::from_str(&data).expect("config parse failed");
        let quote_mints = quote_mints
            .into_iter()
            .filter_map(|s| Pubkey::from_str(&s).ok())
            .collect();
        let tg = telegram.into_iter().next().expect("telegram config missing");
        Self {
            rpc_url,
            ws_url,
            out_dir,
            quote_mints,
            probe_amount,
            policy,
            hype_cfg: hype,
            tg,
        }
    }
}

#[derive(Deserialize)]
struct RawConfig {
    #[serde(default = "default_rpc_url")]
    rpc_url: String,
    #[serde(default = "default_ws_url")]
    ws_url: String,
    #[serde(default = "default_out_dir")]
    out_dir: PathBuf,
    #[serde(default)]
    quote_mints: Vec<String>,
    #[serde(default = "default_probe_amount")]
    probe_amount: u64,
    #[serde(default)]
    policy: Policy,
    #[serde(default)]
    hype: HypeConfig,
    #[serde(default)]
    telegram: Vec<TgConfig>,
}

fn default_rpc_url() -> String {
    "https://api.mainnet-beta.solana.com".into()
}

fn default_ws_url() -> String {
    "wss://api.mainnet-beta.solana.com".into()
}

fn default_out_dir() -> PathBuf {
    PathBuf::from("./outbox")
}

fn default_probe_amount() -> u64 {
    1_000_000
}

fn spawn_logs_ingestor(bus: Arc<PoolBus>, hype: Arc<HypeAggregator>) {
    tokio::spawn(async move {
        let mut rx = bus.subscribe();
        while let Ok(ev) = rx.recv().await {
            if let PoolEvent::ProgramLog {
                program,
                signature,
                slot,
            } = ev
            {
                let pl = PoolLogEvent {
                    program,
                    pool: program,
                    signature,
                    slot,
                    logs: Vec::new(),
                    ts_ms: current_ms(),
                    trader: None,
                };
                hype.ingest(pl).await;
            }
        }
    });
}

async fn spawn_pool_pipeline(
    bus: Arc<PoolBus>,
    rpc: Arc<RpcClient>,
    tg: TgPublisher,
    sink: FileSink,
    hype: Arc<HypeAggregator>,
    cfg: Config,
) {
    let seen = Arc::new(Mutex::new(LruCache::<Pubkey, u64>::new(
        NonZeroUsize::new(10_000).unwrap(),
    )));
    let mint_cache = Arc::new(Mutex::new(LruCache::<Pubkey, TokenSafetyReport>::new(
        NonZeroUsize::new(20_000).unwrap(),
    )));
    tokio::spawn(async move {
        let mut rx = bus.subscribe();
        while let Ok(ev) = rx.recv().await {
            match ev {
                PoolEvent::AccountNew { info, .. } | PoolEvent::AccountChanged { info, .. } => {
                    if let (Some(mint_a), Some(mint_b)) = (info.base_mint, info.quote_mint) {
                        let pool = info.id.account;
                        let program = info.id.program;
                        let now = current_ms();
                        let ttl = 5 * 60 * 1000;
                        let mut seen_lock = seen.lock().await;
                        let mut process = true;
                        if let Some(ts) = seen_lock.get(&pool).copied() {
                            if now - ts < ttl {
                                process = false;
                            }
                        }
                        if process {
                            seen_lock.put(pool, now);
                        }
                        drop(seen_lock);
                        if !process {
                            continue;
                        }
                        let rpc = rpc.clone();
                        let tg = tg.clone();
                        let sink = sink.clone();
                        let hype = hype.clone();
                        let policy = cfg.policy.clone();
                        let quote_mints = cfg.quote_mints.clone();
                        let probe_amount = cfg.probe_amount;
                        let mint_cache = mint_cache.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_pool_event(
                                rpc,
                                tg,
                                sink,
                                hype,
                                policy,
                                quote_mints,
                                probe_amount,
                                mint_cache,
                                pool,
                                program,
                                mint_a,
                                mint_b,
                                info.fee_bps,
                                info.tick_spacing,
                            )
                            .await
                            {
                                warn!(?e, ?pool, "pipeline failed");
                            }
                        });
                    }
                }
                _ => {}
            }
        }
    });
}

async fn handle_pool_event(
    rpc: Arc<RpcClient>,
    tg: TgPublisher,
    sink: FileSink,
    hype: Arc<HypeAggregator>,
    policy: Policy,
    quote_mints: Vec<Pubkey>,
    probe_amount: u64,
    mint_cache: Arc<Mutex<LruCache<Pubkey, TokenSafetyReport>>>,
    pool: Pubkey,
    program: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    fee_bps: Option<u16>,
    tick_spacing: Option<u16>,
) -> Result<()> {
    let epoch = rpc.get_epoch_info().map(|e| e.epoch).unwrap_or(0);
    let (rep_a, rep_b) = {
        let rpc_a = rpc.clone();
        let cache_a = mint_cache.clone();
        let policy_a = policy.clone();
        let fut_a = async move {
            let mut cache = cache_a.lock().await;
            if let Some(r) = cache.get(&mint_a).cloned() {
                return Ok::<TokenSafetyReport, anyhow::Error>(r);
            }
            drop(cache);
            let r = analyze_mint(&*rpc_a, &mint_a, epoch, probe_amount, true, &policy_a).await?;
            let mut cache = cache_a.lock().await;
            cache.put(mint_a, r.clone());
            Ok::<TokenSafetyReport, anyhow::Error>(r)
        };
        let rpc_b = rpc.clone();
        let cache_b = mint_cache.clone();
        let policy_b = policy.clone();
        let fut_b = async move {
            let mut cache = cache_b.lock().await;
            if let Some(r) = cache.get(&mint_b).cloned() {
                return Ok::<TokenSafetyReport, anyhow::Error>(r);
            }
            drop(cache);
            let r = analyze_mint(&*rpc_b, &mint_b, epoch, probe_amount, true, &policy_b).await?;
            let mut cache = cache_b.lock().await;
            cache.put(mint_b, r.clone());
            Ok::<TokenSafetyReport, anyhow::Error>(r)
        };
        tokio::try_join!(fut_a, fut_b)?
    };

    info!(?pool, "token decode done");
    let decimals_a = rep_a.decimals;
    let decimals_b = rep_b.decimals;
    let input = PoolInput {
        program,
        pool,
        mint_a,
        mint_b,
        decimals_a,
        decimals_b,
        vault_a: None,
        vault_b: None,
        sqrt_price_x64: None,
        is_clmm: false,
        quote_mints,
    };
    let liq = match compute_quick(&*rpc, &input) {
        Ok(v) => {
            info!(?pool, "liq computed");
            Some(v)
        }
        Err(e) => {
            warn!(?e, ?pool, "liq failed");
            None
        }
    };
    let hype_snap = hype.snapshot(&pool).await;
    let bundle = PoolTokenBundle {
        pool,
        program,
        token_a: rep_a.clone(),
        token_b: rep_b.clone(),
        fee_bps,
        tick_spacing,
        ts_ms: current_ms(),
    };
    let alert = EnrichedPoolAlert {
        bundle,
        liq,
        hype: hype_snap,
    };
    if let Err(e) = sink.write_json("alerts_enriched", &alert).await {
        warn!(?e, ?pool, "file sink error");
    }
    if let Err(e) = tg.send_enriched_alert(&alert).await {
        warn!(?e, ?pool, "tg send failed");
        let err = serde_json::json!({"pool": pool.to_string(), "err": format!("{}", e)});
        let _ = sink.write_json("errors", &err).await;
    } else {
        info!(?pool, "tg sent");
    }
    Ok(())
}

fn should_process(cache: &mut LruCache<Pubkey, u64>, key: Pubkey, now: u64, ttl: u64) -> bool {
    if let Some(ts) = cache.get(&key).copied() {
        if now - ts < ttl {
            return false;
        }
    }
    cache.put(key, now);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_recent() {
        let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());
        let key = Pubkey::new_unique();
        assert!(should_process(&mut cache, key, 0, 1000));
        assert!(!should_process(&mut cache, key, 500, 1000));
        assert!(should_process(&mut cache, key, 2000, 1000));
    }
}
