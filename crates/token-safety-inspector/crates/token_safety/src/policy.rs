use crate::report::{SafetyReport};
use serde::{Serialize, Deserialize};

/// Policy controls how [`SafetyReport`]s are evaluated into a decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub require_freeze_authority_none: bool,
    pub forbid_non_transferable: bool,
    pub forbid_default_frozen: bool,
    pub forbid_permanent_delegate: bool,
    pub forbid_transfer_hook: bool,
    pub forbid_confidential: bool,
    pub max_fee_bps: u16,
    pub max_fee_absolute: u64,
    pub allow_mint_authority: bool,
    pub forbid_memo_required_if_route_has_no_memo: bool,
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
            max_fee_bps: 100,
            max_fee_absolute: 0, // later scaled by decimals
            allow_mint_authority: false,
            forbid_memo_required_if_route_has_no_memo: true,
            forbid_mint_close_authority: false,
        }
    }
}

/// Decision produced by evaluating a [`SafetyReport`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub safe: bool,
    pub reasons: Vec<String>,
    pub warnings: Vec<String>,
}

impl Decision {
    fn new() -> Self {
        Self { safe: true, reasons: vec![], warnings: vec![] }
    }
}

impl Policy {
    /// Evaluate a [`SafetyReport`] into a [`Decision`].
    pub fn evaluate(&self, report: &SafetyReport, route_supports_memo: bool) -> Decision {
        let mut d = Decision::new();

        if !report.flags.freeze_authority_none && self.require_freeze_authority_none {
            d.safe = false;
            d.reasons.push("freeze_authority_present".into());
        }
        if !report.flags.mint_authority_none {
            if self.allow_mint_authority {
                d.warnings.push("mint_authority_present".into());
            } else {
                d.safe = false;
                d.reasons.push("mint_authority_present".into());
            }
        }
        if report.flags.non_transferable && self.forbid_non_transferable {
            d.safe = false;
            d.reasons.push("non_transferable".into());
        }
        if report.flags.default_frozen && self.forbid_default_frozen {
            d.safe = false;
            d.reasons.push("default_frozen".into());
        }
        if report.flags.permanent_delegate && self.forbid_permanent_delegate {
            d.safe = false;
            d.reasons.push("permanent_delegate".into());
        }
        if report.flags.transfer_hook && self.forbid_transfer_hook {
            d.safe = false;
            d.reasons.push("transfer_hook".into());
        }
        if report.flags.memo_required && self.forbid_memo_required_if_route_has_no_memo && !route_supports_memo {
            d.safe = false;
            d.reasons.push("memo_required".into());
        }
        if report.flags.confidential && self.forbid_confidential {
            d.safe = false;
            d.reasons.push("confidential".into());
        }
        if report.flags.mint_close_authority {
            if self.forbid_mint_close_authority {
                d.safe = false;
                d.reasons.push("mint_close_authority".into());
            } else {
                d.warnings.push("mint_close_authority".into());
            }
        }

        if let Some(fee) = &report.transfer_fee {
            if fee.fee_bps > self.max_fee_bps {
                d.safe = false;
                d.reasons.push("transfer_fee_bps_exceeds_policy".into());
            }
            if fee.max_fee > self.max_fee_absolute && self.max_fee_absolute > 0 {
                d.safe = false;
                d.reasons.push("transfer_fee_max_exceeds_policy".into());
            }
        }

        d
    }
}

