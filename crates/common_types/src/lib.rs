use serde::{Serialize,Deserialize};
use solana_sdk::pubkey::Pubkey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolEventCreated {
  pub program: Pubkey,
  pub pool: Pubkey,
  pub token_a_mint: Pubkey,
  pub token_b_mint: Pubkey,
  pub fee_bps: Option<u16>,
  pub tick_spacing: Option<u16>,
  pub ts_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TokenProgramKind { TokenV1, Token2022, Other(String) }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenExtensionFlags {
  pub non_transferable: bool,
  pub default_frozen: bool,
  pub permanent_delegate: bool,
  pub transfer_hook: bool,
  pub memo_required: bool,
  pub confidential: bool,
  pub mint_close_authority: bool,
  pub transfer_fee_bps: Option<u16>,
  pub transfer_fee_max: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSafetyReport {
  pub mint: Pubkey,
  pub program: TokenProgramKind,
  pub decimals: u8,
  pub supply: u64,
  pub mint_authority_none: bool,
  pub freeze_authority_none: bool,
  pub flags: TokenExtensionFlags,
  pub decision_safe: bool,
  pub reasons: Vec<String>,
  pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolTokenBundle {
  pub pool: Pubkey,
  pub program: Pubkey,
  pub token_a: TokenSafetyReport,
  pub token_b: TokenSafetyReport,
  pub fee_bps: Option<u16>,
  pub tick_spacing: Option<u16>,
  pub ts_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickLiq {
  pub price_ab: Option<f64>,
  pub reserves_a: u64,
  pub reserves_b: u64,
  pub tvl_quote: Option<f64>,
  pub quote_liquidity: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypeSnapshot {
  pub swaps_60s: u32,
  pub buy_sell_ratio: f32,
  pub unique_traders_60s: u32,
  pub lp_net_300s: i32,
  pub score: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedPoolAlert {
  pub bundle: PoolTokenBundle,
  pub liq: Option<QuickLiq>,
  pub hype: Option<HypeSnapshot>,
}

