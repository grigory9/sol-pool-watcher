use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{account::Account, pubkey::Pubkey};
use once_cell::sync::Lazy;
use std::str::FromStr;

static TOKEN_2022_PROGRAM_ID: Lazy<Pubkey> = Lazy::new(|| {
    Pubkey::from_str("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb").unwrap()
});

use crate::decoders::TokenIntrospectionProvider;

/// Trait for fetching mint accounts and current epoch information.
pub trait MintFetcher: Send + Sync {
    fn get_account(&self, mint: &Pubkey) -> Result<Account>;
    fn get_epoch(&self) -> Result<u64>;
}

impl MintFetcher for RpcClient {
    fn get_account(&self, mint: &Pubkey) -> Result<Account> {
        Ok(RpcClient::get_account(self, mint)?)
    }

    fn get_epoch(&self) -> Result<u64> {
        Ok(RpcClient::get_epoch_info(self)?.epoch)
    }
}

/// Provider that inspects token metadata using direct account owner checks.
pub struct TokenSafetyProvider<F: MintFetcher> {
    rpc: F,
}

impl<F: MintFetcher> TokenSafetyProvider<F> {
    pub fn new(rpc: F) -> Self {
        Self { rpc }
    }
}

impl<F: MintFetcher> TokenIntrospectionProvider for TokenSafetyProvider<F> {
    fn is_token2022(&self, mint: &Pubkey) -> Result<bool> {
        let account = self.rpc.get_account(mint)?;
        Ok(account.owner == *TOKEN_2022_PROGRAM_ID)
    }
}
