use std::collections::HashSet;
use pool_watcher::decoders::{self, orca_whirl, raydium_clmm, TokenIntrospectionProvider};
use pool_watcher::types::DexKind;
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

struct MockTokenProvider { tokens: HashSet<Pubkey> }

impl TokenIntrospectionProvider for MockTokenProvider {
    fn is_token2022(&self, mint: &Pubkey) -> anyhow::Result<bool> {
        Ok(self.tokens.contains(mint))
    }
}

#[test]
fn test_decode_pool_token2022() {
    let program = Pubkey::new_unique();
    let account = Pubkey::new_unique();
    let token_a = Pubkey::new_unique();
    let token_b = Pubkey::new_unique();
    let mut data = vec![0u8; 200];
    data[69..101].copy_from_slice(token_a.as_ref());
    data[149..181].copy_from_slice(token_b.as_ref());
    data[9..11].copy_from_slice(&3u16.to_le_bytes());
    data[13..15].copy_from_slice(&5u16.to_le_bytes());
    let mut set = HashSet::new();
    set.insert(token_a);
    let provider = MockTokenProvider { tokens: set };
    let info = decoders::decode_pool(
        DexKind::OrcaWhirlpools,
        program,
        account,
        &data,
        &provider,
    ).expect("decode");
    assert!(info.is_token2022_base);
    assert!(!info.is_token2022_quote);
}

#[test]
fn test_decode_pool_raydium_cpmm_kind() {
    let program = Pubkey::new_unique();
    let account = Pubkey::new_unique();
    let cfg_account = Pubkey::new_unique();
    let token_a = Pubkey::new_unique();
    let token_b = Pubkey::new_unique();

    let mut cfg = vec![0u8; 117];
    cfg[47..51].copy_from_slice(&300u32.to_le_bytes());
    raydium_clmm::try_decode(program, cfg_account, &cfg);

    let mut data = vec![0u8; 240];
    data[9..41].copy_from_slice(cfg_account.as_ref());
    data[73..105].copy_from_slice(token_a.as_ref());
    data[105..137].copy_from_slice(token_b.as_ref());
    data[235..237].copy_from_slice(&9u16.to_le_bytes());
    let provider = MockTokenProvider { tokens: HashSet::new() };
    let info = decoders::decode_pool(
        DexKind::RaydiumCpmm,
        program,
        account,
        &data,
        &provider,
    ).expect("decode");
    assert_eq!(info.dex, DexKind::RaydiumCpmm);
    assert_eq!(info.base_mint, Some(token_a));
    assert_eq!(info.quote_mint, Some(token_b));
}
