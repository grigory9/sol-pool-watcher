use anyhow::{anyhow, Result};
use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;
use spl_token::solana_program::program_pack::Pack;
use spl_token::state as spl_v1;
use std::str::FromStr;

use crate::extensions::analyze_extensions;
use crate::report::{Flags, ProgramOwner, SafetyReport};

/// Analyze mint account.
pub fn analyze_mint(account: &Account, now_epoch: u64, _probe_amount: u64) -> Result<SafetyReport> {
    let mint_pubkey = Pubkey::default();
    let owner = account.owner;
    let token2022_id = Pubkey::from_str("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb").unwrap();
    let token_v1_id = Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap();

    // Helper to safely decode a mint account. Returns None if data is too short
    // or fails to unpack.
    fn unpack_mint(data: &[u8]) -> Option<spl_v1::Mint> {
        if data.len() < spl_v1::Mint::LEN {
            return None;
        }
        // Only decode the portion of the slice needed by the mint structure
        spl_v1::Mint::unpack_from_slice(&data[..spl_v1::Mint::LEN]).ok()
    }

    if owner == token_v1_id {
        let mint = unpack_mint(&account.data).ok_or_else(|| anyhow!("invalid SPL Token mint"))?;
        let flags = Flags {
            mint_authority_none: mint.mint_authority.is_none(),
            freeze_authority_none: mint.freeze_authority.is_none(),
            ..Flags::default()
        };
        Ok(SafetyReport {
            mint: mint_pubkey,
            program_owner: ProgramOwner::TokenV1,
            decimals: mint.decimals,
            supply: mint.supply,
            flags,
            transfer_fee: None,
            other_extensions: vec![],
        })
    } else if owner == token2022_id {
        let mint = unpack_mint(&account.data).ok_or_else(|| anyhow!("invalid Token-2022 mint"))?;
        let (mut flags, transfer_fee, other_ext) = analyze_extensions(&account.data, now_epoch);
        flags.mint_authority_none = mint.mint_authority.is_none();
        flags.freeze_authority_none = mint.freeze_authority.is_none();
        Ok(SafetyReport {
            mint: mint_pubkey,
            program_owner: ProgramOwner::Token2022,
            decimals: mint.decimals,
            supply: mint.supply,
            flags,
            transfer_fee,
            other_extensions: other_ext,
        })
    } else {
        let mint = unpack_mint(&account.data);
        let decimals = mint.map(|m| m.decimals).unwrap_or(0);
        let supply = mint.map(|m| m.supply).unwrap_or(0);
        Ok(SafetyReport {
            mint: mint_pubkey,
            program_owner: ProgramOwner::Other,
            decimals,
            supply,
            flags: Flags::default(),
            transfer_fee: None,
            other_extensions: vec![],
        })
    }
}
