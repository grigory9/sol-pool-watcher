use solana_sdk::pubkey::Pubkey;
use crate::types::{DexKind, PoolId, PoolInfo};

/// Minimal layout reader for Orca Whirlpools using on-chain account layout.
pub fn try_decode(program: Pubkey, account: Pubkey, data: &[u8]) -> Option<PoolInfo> {
    // Discriminator + account fields; need at least up to token_b
    if data.len() < 181 { return None; }

    const TOKEN_A_OFFSET: usize = 69;
    const TOKEN_B_OFFSET: usize = 149;
    const TICK_SPACING_OFFSET: usize = 9;
    const FEE_RATE_OFFSET: usize = 13;

    let token_a = Pubkey::new_from_array(data.get(TOKEN_A_OFFSET..TOKEN_A_OFFSET+32)?.try_into().ok()?);
    let token_b = Pubkey::new_from_array(data.get(TOKEN_B_OFFSET..TOKEN_B_OFFSET+32)?.try_into().ok()?);
    let tick_spacing = u16::from_le_bytes(data.get(TICK_SPACING_OFFSET..TICK_SPACING_OFFSET+2)?.try_into().ok()?);
    let fee_bps = u16::from_le_bytes(data.get(FEE_RATE_OFFSET..FEE_RATE_OFFSET+2)?.try_into().ok()?);

    Some(PoolInfo {
        dex: DexKind::OrcaWhirlpools,
        id: PoolId { program, account },
        base_mint: Some(token_a),
        quote_mint: Some(token_b),
        fee_bps: Some(fee_bps),
        tick_spacing: Some(tick_spacing),
        lp_mint: None,
        is_token2022_base: false,
        is_token2022_quote: false,
    })
}
