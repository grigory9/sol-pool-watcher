use crate::types::{DexKind, PoolInfo};
use solana_sdk::pubkey::Pubkey;
pub mod orca_whirl;
pub mod raydium_clmm;

// Reuse existing token decoding utilities via crate::token or a passed-in trait.
pub trait TokenIntrospectionProvider: Send + Sync {
    fn is_token2022(&self, mint: &Pubkey) -> anyhow::Result<bool>;
}

pub fn decode_pool(
    kind: DexKind,
    program: Pubkey,
    account: Pubkey,
    data: &[u8],
    token: &dyn TokenIntrospectionProvider,
) -> Option<PoolInfo> {
    let mut info = match kind {
        DexKind::OrcaWhirlpools => crate::decoders::orca_whirl::try_decode(program, account, data),
        DexKind::RaydiumClmm | DexKind::RaydiumCpmm => {
            crate::decoders::raydium_clmm::try_decode(program, account, data)
        }
    }?;
    // ensure the returned info reflects the requested DEX kind
    info.dex = kind;
    info.is_token2022_base = info
        .base_mint
        .map(|m| token.is_token2022(&m).unwrap_or(false))
        .unwrap_or(false);
    info.is_token2022_quote = info
        .quote_mint
        .map(|m| token.is_token2022(&m).unwrap_or(false))
        .unwrap_or(false);
    Some(info)
}
