#![cfg(all(test, feature = "token-tests"))]
use super::*;
use crate::test_fixtures::*;
use crate::policy::Policy;
use solana_sdk::pubkey::Pubkey;

#[tokio::test]
async fn v1_ok() {
    let acc = mk_v1_safe_mint(6);
    let rpc = dummy_rpc_with_account(acc);
    let pol = Policy::default();
    let mint = Pubkey::new_unique();
    let rep = analyze_mint(&rpc, &mint, 0, 1_000, true, &pol).await.unwrap();
    assert!(rep.decision_safe);
}

#[tokio::test]
async fn non_transferable_ban() {
    let acc = mk_22_non_transferable();
    let rpc = dummy_rpc_with_account(acc);
    let pol = Policy::default();
    let mint = Pubkey::new_unique();
    let rep = analyze_mint(&rpc, &mint, 0, 1_000, true, &pol).await.unwrap();
    assert!(!rep.decision_safe);
    assert!(rep.reasons.iter().any(|r| r.contains("non_transferable")));
}

#[tokio::test]
async fn default_frozen_ban() {
    let acc = mk_22_default_frozen();
    let rpc = dummy_rpc_with_account(acc);
    let pol = Policy::default();
    let mint = Pubkey::new_unique();
    let rep = analyze_mint(&rpc, &mint, 0, 1_000, true, &pol).await.unwrap();
    assert!(!rep.decision_safe);
}

#[tokio::test]
async fn transfer_fee_too_high() {
    let acc = mk_22_transfer_fee(250, 10);
    let rpc = dummy_rpc_with_account(acc);
    let pol = Policy::default();
    let mint = Pubkey::new_unique();
    let rep = analyze_mint(&rpc, &mint, 0, 1_000, true, &pol).await.unwrap();
    assert!(!rep.decision_safe);
    assert!(rep.reasons.iter().any(|r| r.contains("transfer_fee")));
}
