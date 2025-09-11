use anyhow::Result;
use common_types::{PoolTokenBundle, TokenExtensionFlags, TokenProgramKind, TokenSafetyReport};
use solana_sdk::pubkey::Pubkey;
use tg_publisher::{TgConfig, TgPublisher};
use tokio::time::{sleep, Duration};

#[tokio::test]
#[ignore]
async fn send_message_to_telegram() -> Result<()> {
    let cfg_str = std::fs::read_to_string("tests/tg_publisher_config.toml")
        .expect("missing test config: tests/tg_publisher_config.toml");
    let cfg: TgConfig = toml::from_str(&cfg_str)?;

    let publisher = TgPublisher::new(cfg)?;

    let token_report = TokenSafetyReport {
        mint: Pubkey::new_unique(),
        program: TokenProgramKind::TokenV1,
        decimals: 0,
        supply: 0,
        mint_authority_none: true,
        freeze_authority_none: true,
        flags: TokenExtensionFlags::default(),
        decision_safe: true,
        reasons: vec!["integration test".to_string()],
        warnings: vec![],
    };
    let bundle = PoolTokenBundle {
        pool: Pubkey::new_unique(),
        program: Pubkey::new_unique(),
        token_a: token_report.clone(),
        token_b: token_report,
        fee_bps: Some(0),
        tick_spacing: Some(1),
        ts_ms: 0,
    };

    publisher.send_pool_bundle(&bundle).await?;
    sleep(Duration::from_secs(1)).await;

    Ok(())
}
