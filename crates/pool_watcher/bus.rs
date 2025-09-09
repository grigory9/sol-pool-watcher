use tokio::sync::broadcast;
use std::sync::Arc;
use crate::types::PoolEvent;

#[derive(Clone)]
pub struct PoolBus {
    tx: broadcast::Sender<PoolEvent>,
}

impl PoolBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }
    pub fn subscribe(&self) -> broadcast::Receiver<PoolEvent> { self.tx.subscribe() }
    pub fn publish(&self, ev: PoolEvent) { let _ = self.tx.send(ev); }
}
pub type SharedPoolBus = Arc<PoolBus>;
