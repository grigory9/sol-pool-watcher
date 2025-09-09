use once_cell::sync::Lazy;
use dashmap::DashMap;
use solana_sdk::pubkey::Pubkey;
use crate::types::{DexKind, PoolId, PoolInfo};

static CONFIG_FEES: Lazy<DashMap<Pubkey, u16>> = Lazy::new(DashMap::new);

pub fn try_decode(program: Pubkey, account: Pubkey, data: &[u8]) -> Option<PoolInfo> {
    const CONFIG_LEN: usize = 117;
    const TRADE_FEE_OFFSET: usize = 47;
    const AMM_CONFIG_OFFSET: usize = 9;
    const TOKEN_BASE_OFFSET: usize = 73;
    const TOKEN_QUOTE_OFFSET: usize = 105;
    const TICK_SPACING_OFFSET: usize = 235;

    if data.len() == CONFIG_LEN {
        let fee = u32::from_le_bytes(data.get(TRADE_FEE_OFFSET..TRADE_FEE_OFFSET+4)?.try_into().ok()?);
        let fee_bps = ((fee as u64 * 10_000) / 1_000_000) as u16;
        CONFIG_FEES.insert(account, fee_bps);
        return None;
    }

    if data.len() <= TICK_SPACING_OFFSET + 2 { return None; }

    let amm_config = Pubkey::new_from_array(data.get(AMM_CONFIG_OFFSET..AMM_CONFIG_OFFSET+32)?.try_into().ok()?);
    let token_base = Pubkey::new_from_array(data.get(TOKEN_BASE_OFFSET..TOKEN_BASE_OFFSET+32)?.try_into().ok()?);
    let token_quote = Pubkey::new_from_array(data.get(TOKEN_QUOTE_OFFSET..TOKEN_QUOTE_OFFSET+32)?.try_into().ok()?);
    let tick_spacing = u16::from_le_bytes(data.get(TICK_SPACING_OFFSET..TICK_SPACING_OFFSET+2)?.try_into().ok()?);
    let fee_bps = CONFIG_FEES.get(&amm_config).map(|v| *v);

    Some(PoolInfo {
        dex: DexKind::RaydiumClmm,
        id: PoolId { program, account },
        base_mint: Some(token_base),
        quote_mint: Some(token_quote),
        fee_bps,
        tick_spacing: Some(tick_spacing),
        lp_mint: None,
        is_token2022_base: false,
        is_token2022_quote: false,
    })
}
