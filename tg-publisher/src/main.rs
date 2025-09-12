use anyhow::Result;
use common_types::EnrichedPoolAlert;
use futures::StreamExt;
use serde::Deserialize;
use tg_publisher::{TgConfig, TgPublisher};
use tokio::time::{sleep, Duration};
use tokio_tungstenite::connect_async;
use tracing::warn;

#[derive(Deserialize)]
struct Config {
    #[serde(flatten)]
    tg: TgConfig,
    #[serde(rename = "ARB_WS_IP")]
    ws_ip: String,
    #[serde(rename = "ARB_WS_PORT")]
    ws_port: u16,
}

impl Config {
    fn from_file(path: &str) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&text)?)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = Config::from_file("tg-publisher.toml")?;
    let url = format!("ws://{}:{}", cfg.ws_ip, cfg.ws_port);
    let tg = TgPublisher::new(cfg.tg.clone())?;
    loop {
        let ws_stream = loop {
            match connect_async(&url).await {
                Ok((stream, _)) => break stream,
                Err(e) => {
                    warn!(?e, "ws connect failed, retrying in 2s");
                    sleep(Duration::from_secs(2)).await;
                }
            }
        };
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
        warn!("ws connection closed, retrying in 2s");
        sleep(Duration::from_secs(2)).await;
    }
}
