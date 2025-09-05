use anyhow::{Result,Context};
use solana_account_decoder::parse_token::spl_token_id_v2;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, account::Account};
use spl_token::state::Mint as MintV1;
use spl_token_2022::{self, state::Mint as Mint22, extension::{StateWithExtensions, ExtensionType, non_transferable::NonTransferable, default_account_state::{DefaultAccountState, AccountState}, permanent_delegate::PermanentDelegate, transfer_hook::TransferHook, memo_transfer::MemoTransfer, confidential_transfer::ConfidentialTransferMint, transfer_fee::TransferFeeConfig, mint_close_authority::MintCloseAuthority}};
use crate::policy::Policy;
use common_types::{TokenSafetyReport, TokenProgramKind, TokenExtensionFlags};

pub mod policy;
#[cfg(test)]
mod test_fixtures;

pub async fn analyze_mint<F: MintFetcher>(rpc: &F, mint: &Pubkey, _now_epoch: u64, _probe_amount: u64, route_supports_memo: bool, policy: &Policy) -> Result<TokenSafetyReport> {
    let acc = rpc.get_account(mint)?;
    let owner = acc.owner;
    let mut report = TokenSafetyReport {
        mint: *mint,
        program: TokenProgramKind::Other(owner.to_string()),
        decimals: 0,
        supply: 0,
        mint_authority_none: true,
        freeze_authority_none: true,
        flags: TokenExtensionFlags::default(),
        decision_safe: true,
        reasons: vec![],
        warnings: vec![],
    };
    if owner == spl_token::id() {
        report.program = TokenProgramKind::TokenV1;
        let mint: MintV1 = MintV1::unpack(&acc.data).context("mint v1 unpack")?;
        report.decimals = mint.decimals;
        report.supply = mint.supply;
        report.mint_authority_none = mint.mint_authority.is_none();
        report.freeze_authority_none = mint.freeze_authority.is_none();
        if policy.require_freeze_authority_none && !report.freeze_authority_none {
            report.decision_safe = false;
            report.reasons.push("freeze_authority".into());
        }
        if !policy.allow_mint_authority && !report.mint_authority_none {
            report.warnings.push("mint_authority".into());
        }
    } else if owner == spl_token_2022::id() {
        report.program = TokenProgramKind::Token2022;
        let st = StateWithExtensions::<Mint22>::unpack(&acc.data).context("mint22 unpack")?;
        report.decimals = st.base.decimals;
        report.supply = st.base.supply;
        report.mint_authority_none = st.base.mint_authority.is_none();
        report.freeze_authority_none = st.base.freeze_authority.is_none();
        if policy.require_freeze_authority_none && !report.freeze_authority_none {
            report.decision_safe = false;
            report.reasons.push("freeze_authority".into());
        }
        if !policy.allow_mint_authority && !report.mint_authority_none {
            report.warnings.push("mint_authority".into());
        }
        let exts = st.get_extension_types()?
            .into_iter().collect::<Vec<_>>();
        for ext in exts {
            match ext {
                ExtensionType::NonTransferable => {
                    report.flags.non_transferable = true;
                    if policy.forbid_non_transferable {
                        report.decision_safe = false;
                        report.reasons.push("non_transferable".into());
                    }
                },
                ExtensionType::DefaultAccountState => {
                    if let Ok(das) = st.get_extension::<DefaultAccountState>() {
                        if das.state == AccountState::Frozen.into() {
                            report.flags.default_frozen = true;
                            if policy.forbid_default_frozen {
                                report.decision_safe = false;
                                report.reasons.push("default_frozen".into());
                            }
                        }
                    }
                },
                ExtensionType::PermanentDelegate => {
                    report.flags.permanent_delegate = true;
                    if policy.forbid_permanent_delegate {
                        report.decision_safe = false;
                        report.reasons.push("permanent_delegate".into());
                    }
                },
                ExtensionType::TransferHook => {
                    report.flags.transfer_hook = true;
                    if policy.forbid_transfer_hook {
                        report.decision_safe = false;
                        report.reasons.push("transfer_hook".into());
                    }
                },
                ExtensionType::MemoTransfer => {
                    report.flags.memo_required = true;
                    if policy.forbid_memo_required_if_route_no_memo && !route_supports_memo {
                        report.decision_safe = false;
                        report.reasons.push("memo_required".into());
                    }
                },
                ExtensionType::ConfidentialTransferMint => {
                    report.flags.confidential = true;
                    if policy.forbid_confidential {
                        report.decision_safe = false;
                        report.reasons.push("confidential".into());
                    }
                },
                ExtensionType::MintCloseAuthority => {
                    report.flags.mint_close_authority = true;
                    if policy.forbid_mint_close_authority {
                        report.decision_safe = false;
                        report.reasons.push("mint_close_authority".into());
                    } else {
                        report.warnings.push("mint_close_authority".into());
                    }
                },
                ExtensionType::TransferFeeConfig => {
                    if let Ok(tf) = st.get_extension::<TransferFeeConfig>() {
                        let fee = tf.get_epoch_fee(0).basis_points; // assume epoch 0 in tests
                        report.flags.transfer_fee_bps = Some(fee);
                        report.flags.transfer_fee_max = Some(tf.get_epoch_fee(0).maximum_fee);
                        if fee > policy.max_fee_bps {
                            report.decision_safe = false;
                            report.reasons.push("transfer_fee".into());
                        } else if let Some(max_abs) = policy.max_fee_abs_units {
                            if tf.get_epoch_fee(0).maximum_fee > max_abs {
                                report.decision_safe = false;
                                report.reasons.push("transfer_fee".into());
                            }
                        }
                    }
                },
                _ => {}
            }
        }
    }
    if report.reasons.is_empty() {
        report.decision_safe = true;
    }
    Ok(report)
}

pub trait MintFetcher {
    fn get_account(&self, mint: &Pubkey) -> Result<Account>;
}

impl MintFetcher for RpcClient {
    fn get_account(&self, mint: &Pubkey) -> Result<Account> {
        Ok(self.get_account(mint)? )
    }
}

pub mod policy {
    #[derive(Debug)]
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
}
