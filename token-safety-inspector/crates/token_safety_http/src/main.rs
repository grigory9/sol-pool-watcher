use std::{net::SocketAddr, sync::Arc, time::{Duration, Instant}};

use axum::{Router, routing::{get, post}, Json, extract::State, http::{StatusCode, HeaderMap}};
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;
use tracing::info;
use prometheus::{Encoder, TextEncoder, IntCounterVec, opts, Registry};
use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

use token_safety::{Policy, SafetyReport, Decision, report::TransferFeeInfo};

#[derive(Clone)]
struct AppState {
    rpc: Arc<RpcClient>,
    policy: Arc<RwLock<Policy>>,
    cache: Arc<RwLock<std::collections::HashMap<Pubkey, (SafetyReport, Instant)>>>,
    ttl: Duration,
    metrics_req: IntCounterVec,
    metrics_decisions: IntCounterVec,
    admin_token: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();

    let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    let bind_addr: SocketAddr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string()).parse().expect("bind");
    let admin_token = std::env::var("ADMIN_TOKEN").ok();
    let ttl = Duration::from_secs(300);

    let rpc = Arc::new(RpcClient::new(rpc_url));

    let metrics_req = IntCounterVec::new(opts!("requests_total", "requests"), &["endpoint"])?;
    let metrics_decisions = IntCounterVec::new(opts!("decisions", "decisions"), &["safe"])?;
    let registry = Registry::new();
    registry.register(Box::new(metrics_req.clone()))?;
    registry.register(Box::new(metrics_decisions.clone()))?;

    let state = AppState {
        rpc,
        policy: Arc::new(RwLock::new(Policy::default())),
        cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        ttl,
        metrics_req,
        metrics_decisions,
        admin_token,
    };
    let registry = Arc::new(registry);

    let app = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/policy", get(get_policy).put(put_policy))
        .route("/v1/analyze", post(analyze))
        .route("/v1/simulate-sell", post(simulate_sell))
        .route("/metrics", get(move |State(state): State<AppState>| metrics(State(state), registry.clone())) )
        .with_state(state);

    info!(%bind_addr, "listening");
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok"}))
}

async fn get_policy(State(state): State<AppState>) -> Json<Policy> {
    Json(state.policy.read().await.clone())
}

async fn put_policy(State(state): State<AppState>, headers: HeaderMap, Json(new): Json<Policy>) -> Result<Json<Policy>, StatusCode> {
    if let Some(expected) = &state.admin_token {
        let auth_ok = headers.get(axum::http::header::AUTHORIZATION).and_then(|v| v.to_str().ok()).map(|s| s == format!("Bearer {}", expected)).unwrap_or(false);
        if !auth_ok { return Err(StatusCode::UNAUTHORIZED); }
    } else {
        return Err(StatusCode::UNAUTHORIZED);
    }
    *state.policy.write().await = new.clone();
    Ok(Json(new))
}

#[derive(Deserialize)]
struct AnalyzeRequest {
    mint: String,
    probe_amount: u64,
    route_supports_memo: bool,
}

#[derive(Serialize)]
struct AnalyzeResponse {
    mint: String,
    program_owner: String,
    decimals: u8,
    supply: u64,
    flags: token_safety::Flags,
    decision: Decision,
    transfer_fee: Option<TransferFeeInfo>,
    other_extensions: Vec<String>,
}

async fn analyze(State(state): State<AppState>, Json(req): Json<AnalyzeRequest>) -> Result<Json<AnalyzeResponse>, StatusCode> {
    state.metrics_req.with_label_values(&["analyze"]).inc();
    let mint_pubkey = Pubkey::from_str(&req.mint).map_err(|_| StatusCode::BAD_REQUEST)?;

    // caching
    let now = Instant::now();
    if let Some((report, ts)) = state.cache.read().await.get(&mint_pubkey).cloned() {
        if now.duration_since(ts) < state.ttl {
            let policy = state.policy.read().await.clone();
            let decision = token_safety::is_safe(&report, &policy, req.route_supports_memo);
            state.metrics_decisions.with_label_values(&[if decision.safe {"true"} else {"false"}]).inc();
            return Ok(Json(AnalyzeResponse { mint: req.mint, program_owner: format!("{:?}", report.program_owner).to_lowercase(), decimals: report.decimals, supply: report.supply, flags: report.flags, decision, transfer_fee: report.transfer_fee, other_extensions: report.other_extensions }));
        }
    }

    let account = token_safety::fetch_mint(&state.rpc, &mint_pubkey).await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let epoch = token_safety::fetch_epoch(&state.rpc).await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let mut report = token_safety::analyze_mint(&account, epoch, req.probe_amount).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    report.mint = mint_pubkey;

    state.cache.write().await.insert(mint_pubkey, (report.clone(), now));

    let policy = state.policy.read().await.clone();
    let decision = token_safety::is_safe(&report, &policy, req.route_supports_memo);
    state.metrics_decisions.with_label_values(&[if decision.safe {"true"} else {"false"}]).inc();

    Ok(Json(AnalyzeResponse {
        mint: req.mint,
        program_owner: format!("{:?}", report.program_owner).to_lowercase(),
        decimals: report.decimals,
        supply: report.supply,
        flags: report.flags,
        decision,
        transfer_fee: report.transfer_fee,
        other_extensions: report.other_extensions,
    }))
}

#[derive(Deserialize)]
struct SimRequest {
    pool_program: String,
    pool_account: String,
    mint_in: String,
    mint_out: String,
    amount_in: u64,
    slippage_bps: u16,
}

async fn simulate_sell(State(state): State<AppState>, Json(req): Json<SimRequest>) -> Result<Json<token_safety::sim::SimResult>, StatusCode> {
    state.metrics_req.with_label_values(&["simulate"]).inc();
    use std::str::FromStr;
    let result = token_safety::sim::simulate_sell(
        &state.rpc,
        Pubkey::from_str(&req.pool_program).map_err(|_| StatusCode::BAD_REQUEST)?,
        Pubkey::from_str(&req.pool_account).map_err(|_| StatusCode::BAD_REQUEST)?,
        Pubkey::from_str(&req.mint_in).map_err(|_| StatusCode::BAD_REQUEST)?,
        Pubkey::from_str(&req.mint_out).map_err(|_| StatusCode::BAD_REQUEST)?,
        req.amount_in,
        req.slippage_bps,
    ).await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    Ok(Json(result))
}

async fn metrics(State(_state): State<AppState>, registry: Arc<Registry>) -> Result<(StatusCode, String), StatusCode> {
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut buf = Vec::new();
    encoder.encode(&metric_families, &mut buf).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::OK, String::from_utf8(buf).unwrap_or_default()))
}

