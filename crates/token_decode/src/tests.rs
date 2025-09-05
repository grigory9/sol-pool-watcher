#![cfg(test)]
use super::*;
use crate::test_fixtures::*;
use solana_sdk::{pubkey::Pubkey, account::Account};

struct DummyRpc { acc: Account }
impl AccountFetcher for DummyRpc {
    fn get_account(&self, _key: &Pubkey) -> Result<Account> { Ok(self.acc.clone()) }
}
fn dummy_rpc_with_account(acc: Account) -> DummyRpc { DummyRpc { acc } }

#[test]
fn v1_ok() {
    let acc = mk_v1_safe_mint(6);
    let rpc = dummy_rpc_with_account(acc);
    let pol = Policy::default();
    let mint = Pubkey::new_unique();
    let rep = analyze_mint(&rpc, &mint, 0, 1_000, true, &pol).unwrap();
    assert!(rep.decision_safe);
}

#[test]
fn non_transferable_ban() {
    let acc = mk_22_non_transferable();
    let rpc = dummy_rpc_with_account(acc);
    let pol = Policy::default();
    let mint = Pubkey::new_unique();
    let rep = analyze_mint(&rpc, &mint, 0, 1_000, true, &pol).unwrap();
    assert!(!rep.decision_safe);
    assert!(rep.reasons.iter().any(|r| r.contains("non_transferable")));
}

#[test]
fn default_frozen_ban() {
    let acc = mk_22_default_frozen();
    let rpc = dummy_rpc_with_account(acc);
    let pol = Policy::default();
    let mint = Pubkey::new_unique();
    let rep = analyze_mint(&rpc, &mint, 0, 1_000, true, &pol).unwrap();
    assert!(!rep.decision_safe);
}

#[test]
fn transfer_fee_high() {
    let acc = mk_22_transfer_fee(250, 1000);
    let rpc = dummy_rpc_with_account(acc);
    let pol = Policy::default();
    let mint = Pubkey::new_unique();
    let rep = analyze_mint(&rpc, &mint, 0, 1_000, true, &pol).unwrap();
    assert!(!rep.decision_safe);
}

