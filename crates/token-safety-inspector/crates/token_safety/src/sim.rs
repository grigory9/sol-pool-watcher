use anyhow::Result;
use serde::{Serialize, Deserialize};
use solana_sdk::pubkey::Pubkey;
use solana_client::nonblocking::rpc_client::RpcClient;

/// Result of a simulated sell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimResult {
    pub ok: bool,
    pub amount_out: Option<u64>,
    pub units_consumed: Option<u64>,
    pub logs_sample: Vec<String>,
    pub error: Option<String>,
}

/// Simulate a sell through a given pool. Currently unsupported and returns an error.
#[allow(clippy::too_many_arguments)]
pub async fn simulate_sell(
    _rpc: &RpcClient,
    _pool_program: Pubkey,
    _pool_account: Pubkey,
    _mint_in: Pubkey,
    _mint_out: Pubkey,
    _amount_in: u64,
    _slippage_bps: u16,
) -> Result<SimResult> {
    Ok(SimResult { ok: false, amount_out: None, units_consumed: None, logs_sample: vec![], error: Some("unsupported_pool_program".into()) })
}

