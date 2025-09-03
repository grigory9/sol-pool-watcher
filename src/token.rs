use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{account::Account, pubkey::Pubkey};
use token_safety::{analyze_mint, ProgramOwner};

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

/// Provider that inspects token metadata using the `token_safety` crate.
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
        let epoch = self.rpc.get_epoch()?;
        let report = analyze_mint(&account, epoch, 0)?;
        Ok(matches!(report.program_owner, ProgramOwner::Token2022))
    }
}
