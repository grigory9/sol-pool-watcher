pub mod policy;
pub mod report;
pub mod mint_reader;
pub mod extensions;
pub mod sim;

use anyhow::Result;
use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;
use solana_client::nonblocking::rpc_client::RpcClient;

pub use policy::{Policy, Decision};
pub use report::{SafetyReport, Flags, ProgramOwner, effective_transfer_fee, EffectiveFee};

/// Fetch a mint account from the RPC node.
pub async fn fetch_mint(rpc: &RpcClient, mint: &Pubkey) -> Result<Account> {
    let account = rpc.get_account(mint).await?;
    Ok(account)
}

/// Fetch the current epoch from the RPC node.
pub async fn fetch_epoch(rpc: &RpcClient) -> Result<u64> {
    let info = rpc.get_epoch_info().await?;
    Ok(info.epoch)
}

/// Analyze a mint account and produce a [`SafetyReport`].
pub fn analyze_mint(account: &Account, now_epoch: u64, probe_amount: u64) -> Result<SafetyReport> {
    mint_reader::analyze_mint(account, now_epoch, probe_amount)
}

/// Decide if a report is safe according to a policy and route capabilities.
pub fn is_safe(report: &SafetyReport, policy: &Policy, route_supports_memo: bool) -> Decision {
    policy.evaluate(report, route_supports_memo)
}


#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{pubkey::Pubkey, account::Account};
    use std::str::FromStr;
    use spl_token::state as spl_v1;
    use spl_token::solana_program::program_pack::Pack;

    fn create_v1_mint() -> Account {
        use spl_token::solana_program::program_option::COption;
        let mint = spl_v1::Mint {
            mint_authority: COption::None,
            supply: 0,
            decimals: 6,
            is_initialized: true,
            freeze_authority: COption::None,
        };
        let mut data = vec![0u8; spl_v1::Mint::LEN];
        spl_v1::Mint::pack(mint, &mut data).unwrap();
        let program_id = Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap();
        Account { lamports: 0, data, owner: program_id, executable: false, rent_epoch: 0 }
    }

    fn create_2022_mint_non_transferable() -> Account {
        let base_len = spl_v1::Mint::LEN;
        let mut data = vec![0u8; base_len + 4];
        {
            use spl_token::solana_program::program_option::COption;
            let mint = spl_v1::Mint {
                mint_authority: COption::None,
                supply: 0,
                decimals: 6,
                is_initialized: true,
                freeze_authority: COption::None,
            };
            spl_v1::Mint::pack(mint, &mut data[0..base_len]).unwrap();
        }
        // append TLV for NonTransferable
        data[base_len..base_len+2].copy_from_slice(&9u16.to_le_bytes());
        data[base_len+2..base_len+4].copy_from_slice(&0u16.to_le_bytes());
        let program_id = Pubkey::from_str("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb").unwrap();
        Account { lamports: 0, data, owner: program_id, executable: false, rent_epoch: 0 }
    }

    #[tokio::test]
    async fn decision_safe_v1() {
        let account = create_v1_mint();
        let report = analyze_mint(&account, 0, 0).unwrap();
        let policy = Policy::default();
        let d = is_safe(&report, &policy, false);
        assert!(d.safe);
    }

    #[tokio::test]
    async fn detect_non_transferable() {
        let account = create_2022_mint_non_transferable();
        let report = analyze_mint(&account, 0, 0).unwrap();
        assert!(report.flags.non_transferable);
        let policy = Policy::default();
        let d = is_safe(&report, &policy, false);
        assert!(!d.safe);
    }

    #[tokio::test]
    async fn transfer_fee_policy() {
        // Build a report manually with high fee
        let report = SafetyReport {
            mint: Pubkey::default(),
            program_owner: ProgramOwner::Token2022,
            decimals: 6,
            supply: 0,
            flags: Flags { mint_authority_none: true, freeze_authority_none: true, ..Flags::default() },
            transfer_fee: Some(crate::report::TransferFeeInfo { epoch: 0, fee_bps: 200, max_fee: 0 }),
            other_extensions: vec![],
        };
        let policy = Policy::default();
        let d = is_safe(&report, &policy, false);
        assert!(!d.safe);
    }
}

