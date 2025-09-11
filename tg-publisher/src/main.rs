use anyhow::Result;
use common_types::EnrichedPoolAlert;
use futures::StreamExt;
use tg_publisher::TgPublisher;
use tokio_tungstenite::connect_async;
use tracing::warn;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let url = std::env::var("ARB_WS_URL").unwrap_or_else(|_| "ws://127.0.0.1:9001".to_string());
    let tg = TgPublisher::new_from_env()?;
    let (ws_stream, _) = connect_async(&url).await?;
    let (_, mut read) = ws_stream.split();
    while let Some(msg) = read.next().await {
        match msg {
            Ok(m) if m.is_text() => {
                let text = m.into_text()?;
                match serde_json::from_str::<EnrichedPoolAlert>(&text) {
                    Ok(alert) => {
                        if let Err(e) = tg.send_enriched_alert(&alert).await {
                            warn!(?e, "tg send failed");
                        }
                    }
                    Err(e) => warn!(?e, "parse failed"),
                }
            }
            Ok(_) => {}
            Err(e) => {
                warn!(?e, "ws error");
                break;
            }
        }
    }
    Ok(())
}
