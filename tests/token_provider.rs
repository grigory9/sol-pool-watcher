use pool_watcher::token::{MintFetcher, TokenSafetyProvider};
use pool_watcher::TokenIntrospectionProvider;
use solana_sdk::{account::Account, pubkey::Pubkey};
use spl_token::solana_program::program_pack::Pack;
use spl_token::state as spl_v1;
use std::str::FromStr;

#[derive(Clone)]
struct MockRpc {
    account: Account,
}

impl MintFetcher for MockRpc {
    fn get_account(&self, _mint: &Pubkey) -> anyhow::Result<Account> {
        Ok(self.account.clone())
    }

    fn get_epoch(&self) -> anyhow::Result<u64> {
        Ok(0)
    }
}

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
    Account {
        lamports: 0,
        data,
        owner: program_id,
        executable: false,
        rent_epoch: 0,
    }
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
    data[base_len..base_len + 2].copy_from_slice(&9u16.to_le_bytes());
    data[base_len + 2..base_len + 4].copy_from_slice(&0u16.to_le_bytes());
    let program_id = Pubkey::from_str("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb").unwrap();
    Account {
        lamports: 0,
        data,
        owner: program_id,
        executable: false,
        rent_epoch: 0,
    }
}

#[test]
fn detects_token2022_mints() {
    let rpc = MockRpc {
        account: create_2022_mint_non_transferable(),
    };
    let provider = TokenSafetyProvider::new(rpc);
    let mint = Pubkey::new_unique();
    assert!(provider.is_token2022(&mint).unwrap());
}

#[test]
fn detects_token_v1_mints() {
    let rpc = MockRpc {
        account: create_v1_mint(),
    };
    let provider = TokenSafetyProvider::new(rpc);
    let mint = Pubkey::new_unique();
    assert!(!provider.is_token2022(&mint).unwrap());
}
