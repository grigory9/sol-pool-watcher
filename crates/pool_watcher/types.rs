use serde::{Serialize,Deserialize};
use solana_sdk::pubkey::Pubkey;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DexKind { OrcaWhirlpools, RaydiumClmm, RaydiumCpmm }

impl Default for DexKind {
    fn default() -> Self { DexKind::OrcaWhirlpools }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PoolId {
    pub program: Pubkey,
    pub account: Pubkey,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PoolInfo {
    pub dex: DexKind,
    pub id: PoolId,
    pub base_mint: Option<Pubkey>,
    pub quote_mint: Option<Pubkey>,
    pub fee_bps: Option<u16>,
    pub tick_spacing: Option<u16>,
    pub lp_mint: Option<Pubkey>,         // if applicable
    pub is_token2022_base: bool,
    pub is_token2022_quote: bool,
}

#[derive(Debug, Clone)]
pub enum PoolEvent {
    SnapshotStarted { program: Pubkey },
    SnapshotFinished { program: Pubkey, count: usize },
    AccountNew { info: PoolInfo, data_len: usize, slot: u64 },
    AccountChanged { info: PoolInfo, data_len: usize, slot: u64 },
    AccountDeleted { id: PoolId, slot: u64 },
    ProgramLog { program: Pubkey, signature: String, slot: u64 },
    ResyncTick { program: Pubkey },
}
