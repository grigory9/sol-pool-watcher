use pool_watcher::decoders::{orca_whirl, raydium_clmm};
use solana_sdk::pubkey::Pubkey;

#[test]
fn test_orca_decode() {
    let program = Pubkey::new_unique();
    let account = Pubkey::new_unique();
    let token_a = Pubkey::new_unique();
    let token_b = Pubkey::new_unique();
    let mut data = vec![0u8; 200];
    data[69..101].copy_from_slice(token_a.as_ref());
    data[149..181].copy_from_slice(token_b.as_ref());
    data[9..11].copy_from_slice(&3u16.to_le_bytes());
    data[13..15].copy_from_slice(&5u16.to_le_bytes());
    let info = orca_whirl::try_decode(program, account, &data).expect("decode");
    assert_eq!(info.base_mint, Some(token_a));
    assert_eq!(info.quote_mint, Some(token_b));
    assert_eq!(info.tick_spacing, Some(3));
    assert_eq!(info.fee_bps, Some(5));
}

#[test]
fn test_raydium_decode() {
    let program = Pubkey::new_unique();
    let account = Pubkey::new_unique();
    let cfg_account = Pubkey::new_unique();
    let token_a = Pubkey::new_unique();
    let token_b = Pubkey::new_unique();
    // feed config with trade fee
    let mut cfg = vec![0u8; 117];
    cfg[47..51].copy_from_slice(&300u32.to_le_bytes());
    raydium_clmm::try_decode(program, cfg_account, &cfg);

    let mut data = vec![0u8; 240];
    data[9..41].copy_from_slice(cfg_account.as_ref());
    data[73..105].copy_from_slice(token_a.as_ref());
    data[105..137].copy_from_slice(token_b.as_ref());
    data[235..237].copy_from_slice(&9u16.to_le_bytes());
    let info = raydium_clmm::try_decode(program, account, &data).expect("decode");
    assert_eq!(info.base_mint, Some(token_a));
    assert_eq!(info.quote_mint, Some(token_b));
    assert_eq!(info.fee_bps, Some(3));
    assert_eq!(info.tick_spacing, Some(9));
}
