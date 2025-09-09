use dashmap::DashMap;
use std::sync::Arc;
use crate::types::{PoolId, PoolInfo};

#[derive(Clone, Default)]
pub struct Inventory {
    // program -> account -> PoolInfo
    inner: Arc<DashMap<String, DashMap<String, PoolInfo>>>,
}
impl Inventory {
    pub fn upsert(&self, info: PoolInfo) {
        let pid = info.id.program.to_string();
        let aid = info.id.account.to_string();
        let map = self.inner.entry(pid).or_insert_with(DashMap::new);
        map.insert(aid, info);
    }
    pub fn remove(&self, id: &PoolId) {
        if let Some(map) = self.inner.get(&id.program.to_string()) {
            let _ = map.remove(&id.account.to_string());
        }
    }
    pub fn count_program(&self, program: &solana_sdk::pubkey::Pubkey) -> usize {
        self.inner.get(&program.to_string()).map(|m| m.len()).unwrap_or(0)
    }
}
