use pool_watcher::{PoolBus, PoolEvent};
use solana_sdk::pubkey::Pubkey;

#[tokio::test]
async fn publish_and_receive() {
    let bus = PoolBus::new(16);
    let mut rx = bus.subscribe();
    bus.publish(PoolEvent::ResyncTick { program: Pubkey::default() });
    let ev = rx.recv().await.unwrap();
    match ev {
        PoolEvent::ResyncTick { .. } => {}
        other => panic!("unexpected event: {:?}", other),
    }
}
