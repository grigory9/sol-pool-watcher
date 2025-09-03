use crate::{
    bus::SharedPoolBus,
    decoders::{decode_pool, TokenIntrospectionProvider},
    inventory::Inventory,
    types::{DexKind, PoolEvent},
};
use serde::Deserialize;
use solana_account_decoder::UiAccountEncoding;
use solana_client::{
    pubsub_client::PubsubClient,
    rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig, RpcTransactionLogsConfig},
    rpc_response::{Response, RpcKeyedAccount},
};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::{str::FromStr, sync::Arc, thread};
use tokio::{
    runtime::Builder,
    time::{sleep, Duration},
};
use tracing::error;

#[derive(Clone, Debug, Deserialize)]
pub struct ProgramConfig {
    pub id: Pubkey,
    pub kind: DexKind,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PoolWatcherConfig {
    pub rpc_url: String,
    pub ws_url: String,
    pub programs: Vec<ProgramConfig>,
    pub periodic_resync_min: u64,
}

impl Default for PoolWatcherConfig {
    fn default() -> Self {
        Self {
            rpc_url: "https://api.mainnet-beta.solana.com".into(),
            ws_url: "wss://api.mainnet-beta.solana.com".into(),
            periodic_resync_min: 30,
            programs: vec![
                ProgramConfig {
                    kind: DexKind::OrcaWhirlpools,
                    id: Pubkey::from_str("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc")
                        .expect("program id"),
                },
                ProgramConfig {
                    kind: DexKind::RaydiumClmm,
                    id: Pubkey::from_str(
                        "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK",
                    )
                    .expect("program id"),
                },
                ProgramConfig {
                    kind: DexKind::RaydiumCpmm,
                    id: Pubkey::from_str(
                        "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8",
                    )
                    .expect("program id"),
                },
            ],
        }
    }
}

pub struct PoolWatcher {
    cfg: PoolWatcherConfig,
    bus: SharedPoolBus,
    inventory: Inventory,
    token: Arc<dyn TokenIntrospectionProvider>,
}

impl PoolWatcher {
    pub fn new(
        cfg: PoolWatcherConfig,
        bus: SharedPoolBus,
        token: Arc<dyn TokenIntrospectionProvider>,
    ) -> Self {
        Self {
            cfg,
            bus,
            inventory: Inventory::default(),
            token,
        }
    }

    /// Spawn in a dedicated OS thread with its own multi-thread Tokio runtime.
    pub fn spawn(self) {
        thread::Builder::new()
            .name("pool-watcher".into())
            .spawn(move || {
                let rt = Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(self.run());
            })
            .expect("spawn pool-watcher");
    }

    async fn run(self) {
        let rpc = RpcClient::new(self.cfg.rpc_url.clone());
        // Initial snapshot
        for prog in &self.cfg.programs {
            self.bus
                .publish(PoolEvent::SnapshotStarted { program: prog.id });
            match self.full_snapshot_program(&rpc, prog).await {
                Ok(count) => self.bus.publish(PoolEvent::SnapshotFinished {
                    program: prog.id,
                    count,
                }),
                Err(e) => error!(err=%e, "snapshot failed"),
            }
        }

        // Subscriptions
        for prog in self.cfg.programs.clone() {
            let ws = self.cfg.ws_url.clone();
            let bus = self.bus.clone();
            let inv = self.inventory.clone();
            let token = self.token.clone();
            let prog_clone = prog.clone();
            tokio::spawn(async move {
                if let Err(e) = subscribe_program(ws, prog_clone, bus, inv, token).await {
                    error!(err=%e, "program subscribe failed");
                }
            });

            let ws2 = self.cfg.ws_url.clone();
            let bus2 = self.bus.clone();
            let prog_clone2 = prog.clone();
            tokio::spawn(async move {
                if let Err(e) = subscribe_logs(ws2, prog_clone2, bus2).await {
                    error!(err=%e, "logs subscribe failed");
                }
            });
        }

        // Periodic resync
        let mins = self.cfg.periodic_resync_min.max(5);
        loop {
            sleep(Duration::from_secs(mins * 60)).await;
            self.bus.publish(PoolEvent::ResyncTick {
                program: Pubkey::default(),
            });
            for prog in &self.cfg.programs {
                self.bus
                    .publish(PoolEvent::SnapshotStarted { program: prog.id });
                match self.full_snapshot_program(&rpc, prog).await {
                    Ok(count) => self.bus.publish(PoolEvent::SnapshotFinished {
                        program: prog.id,
                        count,
                    }),
                    Err(e) => error!(err=%e, "snapshot failed"),
                }
            }
        }
    }

    async fn full_snapshot_program(
        &self,
        rpc: &RpcClient,
        program: &ProgramConfig,
    ) -> anyhow::Result<usize> {
        use solana_client::rpc_config::RpcProgramAccountsConfig;
        let cfg = RpcProgramAccountsConfig {
            filters: None,
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                commitment: Some(CommitmentConfig::processed()),
                data_slice: None,
                min_context_slot: None,
            },
            with_context: None,
            sort_results: None,
        };
        let list = rpc.get_program_accounts_with_config(&program.id, cfg)?;
        let mut count = 0usize;
        for (acc_key, acc) in list {
            let data = acc.data;
            if let Some(info) = decode_pool(
                program.kind,
                program.id,
                acc_key,
                &data,
                self.token.as_ref(),
            ) {
                self.inventory.upsert(info.clone());
                self.bus.publish(PoolEvent::AccountNew {
                    info,
                    data_len: data.len(),
                    slot: 0,
                });
                count += 1;
            }
        }
        Ok(count)
    }
}

async fn subscribe_program(
    ws_url: String,
    program: ProgramConfig,
    bus: SharedPoolBus,
    inventory: Inventory,
    token: Arc<dyn TokenIntrospectionProvider>,
) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move || {
        let cfg = RpcProgramAccountsConfig {
            filters: None,
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                commitment: Some(CommitmentConfig::processed()),
                data_slice: None,
                min_context_slot: None,
            },
            with_context: None,
            sort_results: None,
        };
        let (subscription, receiver) =
            PubsubClient::program_subscribe(&ws_url, &program.id, Some(cfg))?;
        for Response {
            value: RpcKeyedAccount { pubkey, account },
            ..
        } in receiver
        {
            let acc_key = pubkey.parse::<Pubkey>().ok();
            let data_len;
            let data_bytes = match &account.data {
                solana_account_decoder::UiAccountData::Binary(b64, _) => {
                    let v = base64::decode(b64).unwrap_or_default();
                    data_len = v.len();
                    Some(v)
                }
                _ => {
                    data_len = 0;
                    None
                }
            };
            if let (Some(acc_key), Some(bytes)) = (acc_key, data_bytes) {
                if let Some(info) =
                    decode_pool(program.kind, program.id, acc_key, &bytes, token.as_ref())
                {
                    let existed_before = inventory.count_program(&program.id) > 0;
                    inventory.upsert(info.clone());
                    bus.publish(if existed_before {
                        PoolEvent::AccountChanged {
                            info,
                            data_len,
                            slot: 0,
                        }
                    } else {
                        PoolEvent::AccountNew {
                            info,
                            data_len,
                            slot: 0,
                        }
                    });
                }
            }
        }
        drop(subscription);
        Ok::<(), anyhow::Error>(())
    })
    .await??;
    Ok(())
}

async fn subscribe_logs(
    ws_url: String,
    program: ProgramConfig,
    bus: SharedPoolBus,
) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move || {
        let filter = solana_client::rpc_config::RpcTransactionLogsFilter::Mentions(vec![program
            .id
            .to_string()]);
        let (subscription, receiver) = PubsubClient::logs_subscribe(
            &ws_url,
            filter,
            RpcTransactionLogsConfig {
                commitment: Some(CommitmentConfig::processed()),
            },
        )?;
        for Response { value, context } in receiver {
            bus.publish(PoolEvent::ProgramLog {
                program: program.id,
                signature: value.signature,
                slot: context.slot,
            });
        }
        drop(subscription);
        Ok::<(), anyhow::Error>(())
    })
    .await??;
    Ok(())
}
