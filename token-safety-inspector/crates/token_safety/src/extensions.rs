use crate::report::{Flags, TransferFeeInfo};
use spl_token::state::Mint;
use spl_token::solana_program::program_pack::Pack;

// Extension type identifiers from the SPL Token-2022 specification
const EXT_TRANSFER_FEE_CONFIG: u16 = 1;
const EXT_MINT_CLOSE_AUTHORITY: u16 = 3;
const EXT_CONFIDENTIAL_TRANSFER_MINT: u16 = 4;
const EXT_DEFAULT_ACCOUNT_STATE: u16 = 6;
const EXT_MEMO_TRANSFER: u16 = 8;
const EXT_NON_TRANSFERABLE: u16 = 9;
const EXT_PERMANENT_DELEGATE: u16 = 12;
const EXT_TRANSFER_HOOK: u16 = 14;

/// Parse Token-2022 TLV extensions from raw account data.
pub fn analyze_extensions(data: &[u8], _now_epoch: u64) -> (Flags, Option<TransferFeeInfo>, Vec<String>) {
    let mut flags = Flags::default();
    let mut fee = None;
    let mut others = Vec::new();

    let mut i = Mint::LEN;
    while i + 4 <= data.len() {
        let ext_type = u16::from_le_bytes([data[i], data[i + 1]]);
        let len = u16::from_le_bytes([data[i + 2], data[i + 3]]) as usize;
        let start = i + 4;
        let end = start.saturating_add(len);
        if end > data.len() { break; }
        let slice = &data[start..end];
        match ext_type {
            EXT_NON_TRANSFERABLE => flags.non_transferable = true,
            EXT_DEFAULT_ACCOUNT_STATE => {
                if let Some(&state) = slice.get(0) {
                    if state == 2 { flags.default_frozen = true; }
                }
            }
            EXT_PERMANENT_DELEGATE => flags.permanent_delegate = true,
            EXT_TRANSFER_HOOK => flags.transfer_hook = true,
            EXT_MEMO_TRANSFER => {
                if let Some(&b) = slice.get(0) {
                    flags.memo_required = b != 0;
                }
            }
            EXT_CONFIDENTIAL_TRANSFER_MINT => flags.confidential = true,
            EXT_MINT_CLOSE_AUTHORITY => flags.mint_close_authority = true,
            EXT_TRANSFER_FEE_CONFIG => {
                // Parsing full transfer fee config is complex; mark presence only.
                fee = Some(TransferFeeInfo { epoch: 0, fee_bps: 0, max_fee: 0 });
            }
            other => others.push(format!("ext_{}", other)),
        }
        i = end;
    }

    (flags, fee, others)
}

