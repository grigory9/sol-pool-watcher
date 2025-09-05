use std::{collections::{HashMap, HashSet, VecDeque}, time::Duration};
use tokio::sync::RwLock;
use solana_sdk::pubkey::Pubkey;
use common_types::HypeSnapshot;

/// Incoming log event for a pool.
#[derive(Debug, Clone)]
pub struct PoolLogEvent {
    pub program: Pubkey,
    pub pool: Pubkey,
    pub signature: String,
    pub slot: u64,
    pub logs: Vec<String>,
    pub ts_ms: u64,
    pub trader: Option<Pubkey>,
}

/// Config for scoring.
#[derive(Debug, Clone)]
pub struct HypeConfig {
    pub bucket_secs: u64,
    pub window60s: u64,
    pub window300s: u64,
    pub w_swaps: f32,
    pub w_unique: f32,
    pub w_bsr: f32,
    pub w_lp: f32,
}

impl Default for HypeConfig {
    fn default() -> Self {
        Self {
            bucket_secs: 10,
            window60s: 60,
            window300s: 300,
            w_swaps: 0.35,
            w_unique: 0.35,
            w_bsr: 0.20,
            w_lp: 0.10,
        }
    }
}

#[derive(Default, Clone)]
struct Bucket {
    swaps: u32,
    buys: u32,
    sells: u32,
    uniques: HashSet<Pubkey>,
    lp_adds: i32,
    lp_rems: i32,
}

struct PoolSeries {
    buckets: VecDeque<(u64, Bucket)>,
}

impl Default for PoolSeries {
    fn default() -> Self { Self { buckets: VecDeque::new() } }
}

pub struct HypeAggregator {
    cfg: HypeConfig,
    map: RwLock<HashMap<Pubkey, PoolSeries>>,
}

impl HypeAggregator {
    pub fn new(cfg: HypeConfig) -> Self {
        Self { cfg, map: RwLock::new(HashMap::new()) }
    }

    pub async fn ingest(&self, ev: PoolLogEvent) {
        let mut map = self.map.write().await;
        let series = map.entry(ev.pool).or_default();

        let bucket_ts = ev.ts_ms / (self.cfg.bucket_secs * 1000) * (self.cfg.bucket_secs * 1000);
        let horizon_ms = self.cfg.window300s * 1000;
        while let Some((ts, _)) = series.buckets.front() {
            if bucket_ts.saturating_sub(*ts) > horizon_ms { series.buckets.pop_front(); } else { break; }
        }
        if series.buckets.back().map(|(ts, _)| *ts) != Some(bucket_ts) {
            series.buckets.push_back((bucket_ts, Bucket::default()));
        }
        let last = series.buckets.back_mut().unwrap();
        let b = &mut last.1;

        let (is_swap, is_buy, is_sell, lp_add, lp_rem) = classify(&ev.logs);
        if is_swap { b.swaps += 1; }
        if is_buy { b.buys += 1; }
        if is_sell { b.sells += 1; }
        if lp_add { b.lp_adds += 1; }
        if lp_rem { b.lp_rems += 1; }
        if let Some(t) = ev.trader { b.uniques.insert(t); }
    }

    pub async fn snapshot(&self, pool: &Pubkey) -> Option<HypeSnapshot> {
        let map = self.map.read().await;
        let series = map.get(pool)?;
        let now_ms = current_ms();

        let mut swaps_60s = 0u32;
        let mut buys_60s = 0u32;
        let mut sells_60s = 0u32;
        let mut uniq_60s: HashSet<Pubkey> = HashSet::new();
        let mut lp_net_300s: i32 = 0;

        for (ts, b) in series.buckets.iter().rev() {
            let age = now_ms.saturating_sub(*ts);
            if age <= 60_000 {
                swaps_60s += b.swaps;
                buys_60s += b.buys;
                sells_60s += b.sells;
                uniq_60s.extend(b.uniques.iter().cloned());
            }
            if age <= 300_000 {
                lp_net_300s += b.lp_adds - b.lp_rems;
            } else { break; }
        }

        let bsr = if sells_60s == 0 { buys_60s as f32 } else { buys_60s as f32 / sells_60s as f32 };
        let score = score_simple(
            self.cfg.w_swaps, self.cfg.w_unique, self.cfg.w_bsr, self.cfg.w_lp,
            swaps_60s, uniq_60s.len() as u32, bsr, lp_net_300s,
        );

        Some(HypeSnapshot {
            swaps_60s,
            buy_sell_ratio: bsr,
            unique_traders_60s: uniq_60s.len() as u32,
            lp_net_300s,
            score,
        })
    }
}

fn classify(logs: &[String]) -> (bool,bool,bool,bool,bool) {
    let mut is_swap=false; let mut is_buy=false; let mut is_sell=false; let mut lp_add=false; let mut lp_rem=false;
    for l in logs {
        let lo = l.to_ascii_lowercase();
        if lo.contains("swap") { is_swap = true; }
        if lo.contains("increase liquidity") || lo.contains("add liquidity") { lp_add = true; }
        if lo.contains("decrease liquidity") || lo.contains("remove liquidity") { lp_rem = true; }
        if lo.contains("buy ") { is_buy = true; }
        if lo.contains("sell ") { is_sell = true; }
    }
    (is_swap,is_buy,is_sell,lp_add,lp_rem)
}

fn score_simple(w1:f32, w2:f32, w3:f32, w4:f32,
                swaps:u32, uniq:u32, bsr:f32, lp:i32) -> u8 {
    let n_swaps = (swaps as f32 / 50.0).min(1.0);
    let n_unique = (uniq as f32 / 30.0).min(1.0);
    let n_bsr = ((bsr - 0.5) / (3.0 - 0.5)).clamp(0.0, 1.0);
    let n_lp = ((lp as f32) / 20.0).clamp(0.0, 1.0);
    let s = (w1*n_swaps + w2*n_unique + w3*n_bsr + w4*n_lp).clamp(0.0, 1.0) * 100.0;
    s.round() as u8
}

fn current_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

