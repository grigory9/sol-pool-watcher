use anyhow::Result;
use common_types::{TokenExtensionFlags, TokenProgramKind, TokenSafetyReport};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{account::Account, pubkey::Pubkey};
use std::{thread, time::Duration};
use token_safety::{self, policy::Policy, report::ProgramOwner};

pub mod policy {
    pub use token_safety::policy::Policy;
}

#[cfg(test)]
mod test_fixtures;

pub async fn analyze_mint<F: MintFetcher>(
    rpc: &F,
    mint: &Pubkey,
    now_epoch: u64,
    probe_amount: u64,
    route_supports_memo: bool,
    policy: &Policy,
) -> Result<TokenSafetyReport> {
    let acc = get_account_with_retry(|| rpc.get_account(mint))?;
    let ts_report = token_safety::analyze_mint(&acc, now_epoch, probe_amount)?;
    let decision = token_safety::is_safe(&ts_report, policy, route_supports_memo);

    let flags = TokenExtensionFlags {
        non_transferable: ts_report.flags.non_transferable,
        default_frozen: ts_report.flags.default_frozen,
        permanent_delegate: ts_report.flags.permanent_delegate,
        transfer_hook: ts_report.flags.transfer_hook,
        memo_required: ts_report.flags.memo_required,
        confidential: ts_report.flags.confidential,
        mint_close_authority: ts_report.flags.mint_close_authority,
        transfer_fee_bps: ts_report.transfer_fee.as_ref().map(|f| f.fee_bps),
        transfer_fee_max: ts_report.transfer_fee.as_ref().map(|f| f.max_fee),
    };

    let program = match ts_report.program_owner {
        ProgramOwner::TokenV1 => TokenProgramKind::TokenV1,
        ProgramOwner::Token2022 => TokenProgramKind::Token2022,
        ProgramOwner::Other => TokenProgramKind::Other(acc.owner.to_string()),
    };

    Ok(TokenSafetyReport {
        mint: ts_report.mint,
        program,
        decimals: ts_report.decimals,
        supply: ts_report.supply,
        mint_authority_none: ts_report.flags.mint_authority_none,
        freeze_authority_none: ts_report.flags.freeze_authority_none,
        flags,
        decision_safe: decision.safe,
        reasons: decision.reasons,
        warnings: decision.warnings,
    })
}

pub trait MintFetcher {
    fn get_account(&self, mint: &Pubkey) -> Result<Account>;
}

impl MintFetcher for RpcClient {
    fn get_account(&self, mint: &Pubkey) -> Result<Account> {
        Ok(RpcClient::get_account(self, mint)?)
    }
}

fn get_account_with_retry<F>(mut f: F) -> Result<Account>
where
    F: FnMut() -> Result<Account>,
{
    const MAX_RETRIES: usize = 5;
    let mut delay = Duration::from_millis(200);
    for attempt in 0..MAX_RETRIES {
        match f() {
            Ok(acc) => return Ok(acc),
            Err(_e) if attempt + 1 < MAX_RETRIES => {
                thread::sleep(delay);
                delay *= 2;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!("retry loop should return or error before this point")
}
