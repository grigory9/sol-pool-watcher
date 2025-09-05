use anyhow::{Result, Context};
use serde::Serialize;
use tokio::{sync::mpsc, time::{sleep, Duration}};
use tracing::{warn};
use reqwest::Client;
use serde_json::json;

use common_types::{PoolTokenBundle, EnrichedPoolAlert};

mod markdown;
use markdown::escape_md_v2;

#[derive(Clone)]
pub struct TgPublisher {
    client: Client,
    api_base: String,
    chat_id: String,
    send_json_attachment: bool,
    queue_tx: mpsc::Sender<Job>,
}

#[derive(Clone, Debug)]
struct Job {
    text: String,
    json_name: Option<String>,
    json_payload: Option<String>,
}

impl TgPublisher {
    pub fn new_from_env() -> Result<Self> {
        let token = std::env::var("TG_BOT_TOKEN").context("TG_BOT_TOKEN not set")?;
        let chat_id = std::env::var("TG_CHANNEL_ID").context("TG_CHANNEL_ID not set")?;
        let send_json_attachment = std::env::var("TG_SEND_JSON_ATTACHMENT").ok().map(|v| v=="1"||v.eq_ignore_ascii_case("true")).unwrap_or(true);
        let (tx, rx) = mpsc::channel::<Job>(1024);
        let s = Self {
            client: Client::builder().build()?,
            api_base: format!("https://api.telegram.org/bot{}", token),
            chat_id,
            send_json_attachment,
            queue_tx: tx,
        };
        s.spawn_worker(rx);
        Ok(s)
    }

    fn spawn_worker(&self, mut rx: mpsc::Receiver<Job>) {
        let client = self.client.clone();
        let api_base = self.api_base.clone();
        let chat_id = self.chat_id.clone();
        let send_json_attachment = self.send_json_attachment;
        tokio::spawn(async move {
            while let Some(job) = rx.recv().await {
                let mut attempt = 0u32;
                loop {
                    attempt += 1;
                    match send_message(&client, &api_base, &chat_id, &job.text).await {
                        Ok(_) => {
                            if send_json_attachment {
                                if let (Some(name), Some(payload)) = (&job.json_name, &job.json_payload) {
                                    if let Err(e) = send_document_json(&client, &api_base, &chat_id, name, payload).await {
                                        warn!(?e, "send_document failed");
                                    }
                                }
                            }
                            break;
                        }
                        Err(e) => {
                            warn!(?e, attempt, "send_message failed");
                            if attempt >=5 { break; }
                            sleep(Duration::from_millis(300*attempt as u64)).await;
                        }
                    }
                }
            }
        });
    }

    pub async fn send_pool_bundle(&self, bundle: &PoolTokenBundle) -> Result<()> {
        let text = format_pool_message(bundle);
        let json_payload = serde_json::to_string_pretty(bundle)?;
        let job = Job {
            text,
            json_name: Some(format!("pool_{}.json", &bundle.pool.to_string()[..8])),
            json_payload: Some(json_payload),
        };
        self.queue_tx.send(job).await.map_err(|_| anyhow::anyhow!("tg queue closed"))
    }

    pub async fn send_enriched_alert(&self, alert: &EnrichedPoolAlert) -> Result<()> {
        let text = format_enriched_message(alert);
        let json_payload = serde_json::to_string_pretty(alert)?;
        let job = Job {
            text,
            json_name: Some(format!("enriched_{}.json", &alert.bundle.pool.to_string()[..8])),
            json_payload: Some(json_payload),
        };
        self.queue_tx.send(job).await.map_err(|_| anyhow::anyhow!("tg queue closed"))
    }
}

async fn send_message(client: &Client, api_base: &str, chat_id: &str, text: &str) -> Result<()> {
    let url = format!("{}/sendMessage", api_base);
    let body = json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "MarkdownV2",
        "disable_web_page_preview": true
    });
    let resp = client.post(&url).json(&body).send().await?;
    if !resp.status().is_success() {
        let s = resp.text().await.unwrap_or_default();
        anyhow::bail!("TG sendMessage status={} body={}", resp.status(), s);
    }
    Ok(())
}

async fn send_document_json(client: &Client, api_base: &str, chat_id: &str, filename: &str, json_payload: &str) -> Result<()> {
    let url = format!("{}/sendDocument", api_base);
    let part = reqwest::multipart::Part::bytes(json_payload.as_bytes().to_vec())
        .file_name(filename.to_string())
        .mime_str("application/json")?;
    let form = reqwest::multipart::Form::new()
        .text("chat_id", chat_id.to_string())
        .part("document", part);
    let resp = client.post(&url).multipart(form).send().await?;
    if !resp.status().is_success() {
        let s = resp.text().await.unwrap_or_default();
        anyhow::bail!("TG sendDocument status={} body={}", resp.status(), s);
    }
    Ok(())
}

fn short(pk: &solana_sdk::pubkey::Pubkey) -> String {
    let s = pk.to_string();
    format!("{}…{}", &s[..4], &s[s.len()-4..])
}

fn format_pool_message(b: &PoolTokenBundle) -> String {
    let a_ok = if b.token_a.decision_safe { "✅" } else { "⚠️" };
    let b_ok = if b.token_b.decision_safe { "✅" } else { "⚠️" };
    let head = format!(
        "🆕 *New Pool*  fee: *{}* bps  tick: *{}*\nPool: `{}`",
        b.fee_bps.map(|v| v.to_string()).unwrap_or_else(|| "n/a".into()),
        b.tick_spacing.map(|v| v.to_string()).unwrap_or_else(|| "n/a".into()),
        b.pool
    );
    let head = escape_md_v2(&head);
    let a_line = escape_md_v2(&format!(
        "{} A `{}` prog={:?} freeze_none={} mint_none={}",
        a_ok, short(&b.token_a.mint), b.token_a.program, b.token_a.freeze_authority_none, b.token_a.mint_authority_none
    ));
    let b_line = escape_md_v2(&format!(
        "{} B `{}` prog={:?} freeze_none={} mint_none={}",
        b_ok, short(&b.token_b.mint), b.token_b.program, b.token_b.freeze_authority_none, b.token_b.mint_authority_none
    ));
    let mut reasons = Vec::new();
    reasons.extend(b.token_a.reasons.iter().cloned());
    reasons.extend(b.token_b.reasons.iter().cloned());
    let reasons = if reasons.is_empty() { "—".to_string() } else { reasons.join(", ") };
    let reasons = escape_md_v2(&reasons);
    format!(
        "{}\n{}\n{}\n*Reasons:* {}",
        head, a_line, b_line, reasons
    )
}

fn format_enriched_message(a: &EnrichedPoolAlert) -> String {
    let b = &a.bundle;
    let head = escape_md_v2(&format!(
        "🆕 *New Pool*\nfee: *{}* bps, tick: *{}*\nPool: `{}`",
        b.fee_bps.map(|v| v.to_string()).unwrap_or_else(|| "n/a".into()),
        b.tick_spacing.map(|v| v.to_string()).unwrap_or_else(|| "n/a".into()),
        b.pool
    ));
    let a_ok = if b.token_a.decision_safe { "✅" } else { "⚠️" };
    let b_ok = if b.token_b.decision_safe { "✅" } else { "⚠️" };
    let a_line = escape_md_v2(&format!(
        "{} A `{}` prog={:?} fee_ext={:?}",
        a_ok, short(&b.token_a.mint), b.token_a.program, b.token_a.flags.transfer_fee_bps
    ));
    let b_line = escape_md_v2(&format!(
        "{} B `{}` prog={:?} fee_ext={:?}",
        b_ok, short(&b.token_b.mint), b.token_b.program, b.token_b.flags.transfer_fee_bps
    ));
    let liq = if let Some(l) = &a.liq {
        let p = l.price_ab.map(|v| format!("{:.6}", v)).unwrap_or_else(|| "n/a".into());
        let tvl = l.tvl_quote.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "n/a".into());
        let ql = l.quote_liquidity.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "n/a".into());
        escape_md_v2(&format!("💧 *Liquidity*\nprice(A/B): {} | reserves: {}/{}\nTVL(q): {} | quote_liq: {}", p, l.reserves_a, l.reserves_b, tvl, ql))
    } else { escape_md_v2("💧 *Liquidity*\nN/A") };
    let hype = if let Some(h) = &a.hype {
        escape_md_v2(&format!(
            "🔥 *Hype*\n60s swaps: {} | unique: {} | B/S: {:.2}\nLP Δ(300s): {} | score: {}/100",
            h.swaps_60s, h.unique_traders_60s, h.buy_sell_ratio, h.lp_net_300s, h.score
        ))
    } else { escape_md_v2("🔥 *Hype*\nN/A") };
    let mut reasons = Vec::new();
    reasons.extend(b.token_a.reasons.iter().cloned());
    reasons.extend(b.token_b.reasons.iter().cloned());
    let reasons = if reasons.is_empty() { "—".to_string() } else { reasons.join(", ") };
    let reasons = escape_md_v2(&reasons);
    format!("{head}\n{a_line}\n{b_line}\n{liq}\n{hype}\n*Reasons:* {reasons}")
}
