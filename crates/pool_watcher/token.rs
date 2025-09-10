use anyhow::Result;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{account::Account, pubkey::Pubkey};
use std::{str::FromStr, thread, time::Duration};

static TOKEN_2022_PROGRAM_ID: Lazy<Pubkey> =
    Lazy::new(|| Pubkey::from_str("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb").unwrap());

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
    cache: DashMap<Pubkey, bool>,
}

impl<F: MintFetcher> TokenSafetyProvider<F> {
    pub fn new(rpc: F) -> Self {
        Self {
            rpc,
            cache: DashMap::new(),
        }
    }

    fn get_account_retry(&self, mint: &Pubkey) -> Result<Account> {
        const MAX_RETRIES: usize = 5;
        let mut delay = Duration::from_millis(200);
        for attempt in 0..MAX_RETRIES {
            match self.rpc.get_account(mint) {
                Ok(acc) => return Ok(acc),
                Err(_e) if attempt + 1 < MAX_RETRIES => {
                    // retry on transient errors with exponential backoff
                    thread::sleep(delay);
                    delay *= 2;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!("retry loop should return or error before this point")
    }
}

impl<F: MintFetcher> TokenIntrospectionProvider for TokenSafetyProvider<F> {
    fn is_token2022(&self, mint: &Pubkey) -> Result<bool> {
        if let Some(v) = self.cache.get(mint) {
            return Ok(*v);
        }
        let account = self.get_account_retry(mint)?;
        let is_2022 = account.owner == *TOKEN_2022_PROGRAM_ID;
        self.cache.insert(*mint, is_2022);
        Ok(is_2022)
    }
}
