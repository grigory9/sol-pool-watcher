#![cfg(test)]
use solana_sdk::{account::Account, pubkey::Pubkey};
use spl_token::state::Mint as MintV1;
use spl_token_2022::{
    extension::{StateWithExtensions, ExtensionType, non_transferable::NonTransferable, default_account_state::{DefaultAccountState, AccountState}, transfer_fee::{TransferFeeConfig, TransferFee}},
    state::Mint as Mint22,
};

pub fn mk_v1_safe_mint(decimals: u8) -> Account {
    let mut mint = MintV1::default();
    mint.decimals = decimals;
    mint.mint_authority = None;
    mint.freeze_authority = None;
    let mut data = vec![0u8; MintV1::get_packed_len()];
    MintV1::pack(mint, &mut data).unwrap();
    Account { lamports: 1_000_000, data, owner: spl_token::id(), executable: false, rent_epoch: 0 }
}

fn mk_22_with<F>(apply: F) -> Account where F: Fn(&mut StateWithExtensions<Mint22>) {
    let exts = vec![
        ExtensionType::DefaultAccountState,
        ExtensionType::NonTransferable,
        ExtensionType::TransferFeeConfig,
    ];
    let space = StateWithExtensions::<Mint22>::get_packed_len_with_extensions(&exts);
    let mut data = vec![0u8; space];
    let mut st = StateWithExtensions::<Mint22>::unpack_unchecked(&data).unwrap();
    st.base = Mint22 { mint_authority: None.into(), supply: 0, decimals: 6, is_initialized: true.into(), freeze_authority: None.into() };
    apply(&mut st);
    st.pack_base_and_extensions_into_slice(&mut data).unwrap();
    Account { lamports: 1_000_000, data, owner: spl_token_2022::id(), executable: false, rent_epoch: 0 }
}

pub fn mk_22_non_transferable() -> Account {
    mk_22_with(|st| { st.init_extension::<NonTransferable>().unwrap(); })
}

pub fn mk_22_default_frozen() -> Account {
    mk_22_with(|st| {
        let mut das = st.init_extension::<DefaultAccountState>().unwrap();
        das.state = AccountState::Frozen.into();
    })
}

pub fn mk_22_transfer_fee(bps: u16, max_fee: u64) -> Account {
    mk_22_with(|st| {
        let mut tf = st.init_extension::<TransferFeeConfig>().unwrap();
        tf.transfer_fee_config.authority = None.into();
        tf.withheld_amount = 0.into();
        let fee = TransferFee { epoch: 0, maximum_fee: max_fee, transfer_fee_basis_points: bps };
        tf.newer_transfer_fee = fee;
        tf.older_transfer_fee = fee;
    })
}

