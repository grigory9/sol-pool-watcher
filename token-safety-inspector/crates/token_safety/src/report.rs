use serde::{Serialize, Deserialize};
use solana_sdk::pubkey::Pubkey;

mod pubkey_serde {
    use std::str::FromStr;
    use serde::{Deserialize, Deserializer, Serializer};
    use solana_sdk::pubkey::Pubkey;

    pub fn serialize<S>(pk: &Pubkey, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&pk.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Pubkey, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Pubkey::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// Which token program owns the mint account.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProgramOwner {
    #[serde(rename = "token-v1")] TokenV1,
    #[serde(rename = "token-2022")] Token2022,
    #[serde(rename = "other")] Other,
}

/// Flags extracted during mint analysis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Flags {
    pub mint_authority_none: bool,
    pub freeze_authority_none: bool,
    pub non_transferable: bool,
    pub default_frozen: bool,
    pub permanent_delegate: bool,
    pub transfer_hook: bool,
    pub memo_required: bool,
    pub confidential: bool,
    pub mint_close_authority: bool,
}

/// Transfer fee details if present.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferFeeInfo {
    pub epoch: u64,
    pub fee_bps: u16,
    pub max_fee: u64,
}

/// Result of analyzing a mint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyReport {
    #[serde(with = "pubkey_serde")]
    pub mint: Pubkey,
    pub program_owner: ProgramOwner,
    pub decimals: u8,
    pub supply: u64,
    pub flags: Flags,
    pub transfer_fee: Option<TransferFeeInfo>,
    pub other_extensions: Vec<String>,
}

/// Computed effective fee for a given amount.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EffectiveFee {
    pub fee_bps: Option<u16>,
    pub fee_abs: Option<u64>,
}

/// Compute the effective fee for the given report.
pub fn effective_transfer_fee(report: &SafetyReport, amount: u64) -> EffectiveFee {
    if let Some(cfg) = &report.transfer_fee {
        let mut fee = amount.saturating_mul(cfg.fee_bps as u64) / 10_000;
        if fee > cfg.max_fee { fee = cfg.max_fee; }
        EffectiveFee { fee_bps: Some(cfg.fee_bps), fee_abs: Some(fee) }
    } else {
        EffectiveFee { fee_bps: None, fee_abs: None }
    }
}

