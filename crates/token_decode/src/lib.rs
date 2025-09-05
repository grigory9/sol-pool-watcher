use anyhow::Result;
use solana_sdk::{pubkey::Pubkey, account::Account, program_pack::Pack};
use solana_client::rpc_client::RpcClient;
use spl_token::state::Mint as MintV1;
use spl_token_2022::{state::{Mint as Mint22, AccountState}, extension::{StateWithExtensions, BaseStateWithExtensions, non_transferable::NonTransferable, default_account_state::DefaultAccountState, transfer_fee::TransferFeeConfig}};

use common_types::{TokenProgramKind, TokenExtensionFlags, TokenSafetyReport};

#[derive(Debug, Clone)]
pub struct Policy {
    pub require_freeze_authority_none: bool,
    pub forbid_non_transferable: bool,
    pub forbid_default_frozen: bool,
    pub forbid_permanent_delegate: bool,
    pub forbid_transfer_hook: bool,
    pub forbid_confidential: bool,
    pub forbid_memo_required_if_route_no_memo: bool,
    pub max_fee_bps: u16,
    pub max_fee_abs_units: Option<u64>,
    pub allow_mint_authority: bool,
    pub forbid_mint_close_authority: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            require_freeze_authority_none: true,
            forbid_non_transferable: true,
            forbid_default_frozen: true,
            forbid_permanent_delegate: true,
            forbid_transfer_hook: true,
            forbid_confidential: true,
            forbid_memo_required_if_route_no_memo: true,
            max_fee_bps: 100,
            max_fee_abs_units: None,
            allow_mint_authority: false,
            forbid_mint_close_authority: false,
        }
    }
}

/// Trait to fetch accounts; implemented for RpcClient and test doubles.
pub trait AccountFetcher {
    fn get_account(&self, key: &Pubkey) -> Result<Account>;
}

impl AccountFetcher for RpcClient {
    fn get_account(&self, key: &Pubkey) -> Result<Account> {
        Ok(self.get_account(key)?)}
}

pub fn analyze_mint<F: AccountFetcher>(
    rpc: &F,
    mint: &Pubkey,
    _now_epoch: u64,
    _probe_amount: u64,
    route_supports_memo: bool,
    policy: &Policy,
) -> Result<TokenSafetyReport> {
    let acc = rpc.get_account(mint)?;
    let mut flags = TokenExtensionFlags::default();
    let mut reasons = Vec::new();
    let mut warnings = Vec::new();

    let (program, decimals, supply, mint_auth_none, freeze_auth_none) = if acc.owner == spl_token::id() {
        let mint_state = MintV1::unpack(&acc.data)?;
        let mint_auth_none = mint_state.mint_authority.is_none();
        let freeze_auth_none = mint_state.freeze_authority.is_none();
        (TokenProgramKind::TokenV1, mint_state.decimals, mint_state.supply, mint_auth_none, freeze_auth_none)
    } else if acc.owner == spl_token_2022::id() {
        let st = StateWithExtensions::<Mint22>::unpack(&acc.data)?;
        let base = st.base;
        let mint_auth_none = base.mint_authority.is_none();
        let freeze_auth_none = base.freeze_authority.is_none();
        if st.get_extension::<NonTransferable>().is_ok() { flags.non_transferable = true; if policy.forbid_non_transferable { reasons.push("non_transferable".into()); } }
        if let Ok(ext) = st.get_extension::<DefaultAccountState>() {
            if AccountState::try_from(ext.state).ok() == Some(AccountState::Frozen) {
                flags.default_frozen = true;
                if policy.forbid_default_frozen { reasons.push("default_frozen".into()); }
            }
        }
        if let Ok(tf) = st.get_extension::<TransferFeeConfig>() {
            let fee = tf.get_epoch_fee(0);
            let fee_bps: u16 = fee.transfer_fee_basis_points.into();
            let fee_max: u64 = fee.maximum_fee.into();
            flags.transfer_fee_bps = Some(fee_bps);
            flags.transfer_fee_max = Some(fee_max);
            if fee_bps > policy.max_fee_bps { reasons.push("transfer_fee_high".into()); }
            if let Some(max_units) = policy.max_fee_abs_units {
                if fee_max > max_units { reasons.push("transfer_fee_max".into()); }
            }
        }
        (TokenProgramKind::Token2022, base.decimals, base.supply, mint_auth_none, freeze_auth_none)
    } else {
        (TokenProgramKind::Other(acc.owner.to_string()), 0u8, 0u64, true, true)
    };

    if policy.require_freeze_authority_none && !freeze_auth_none { reasons.push("freeze_authority".into()); }
    if !mint_auth_none && !policy.allow_mint_authority { warnings.push("mint_authority".into()); }

    let decision_safe = reasons.is_empty();

    Ok(TokenSafetyReport {
        mint: *mint,
        program,
        decimals,
        supply,
        mint_authority_none: mint_auth_none,
        freeze_authority_none: freeze_auth_none,
        flags,
        decision_safe,
        reasons,
        warnings,
    })
}

#[cfg(test)]
mod tests;
mod test_fixtures;

