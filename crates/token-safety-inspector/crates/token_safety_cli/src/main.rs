use std::str::FromStr;

use anyhow::Result;
use clap::{Parser, Subcommand};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

use token_safety::{self, Policy};

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[arg(long, default_value = "https://api.mainnet-beta.solana.com")]
    rpc: String,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect a mint for safety
    Inspect { mint: String, #[arg(long)] amount: u64, #[arg(long)] json: bool },
    /// Simulate a sell (currently unsupported)
    SimulateSell { #[arg(long)] pool_program: String, #[arg(long)] pool_account: String, #[arg(long, name="in")] mint_in: String, #[arg(long, name="out")] mint_out: String, #[arg(long)] amount: u64, #[arg(long)] slippage_bps: u16 },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let rpc = RpcClient::new(cli.rpc);

    match cli.command {
        Commands::Inspect { mint, amount, json } => {
            let mint_pk = Pubkey::from_str(&mint)?;
            let account = token_safety::fetch_mint(&rpc, &mint_pk).await?;
            let epoch = token_safety::fetch_epoch(&rpc).await?;
            let mut report = token_safety::analyze_mint(&account, epoch, amount)?;
            report.mint = mint_pk;
            let policy = Policy::default();
            let decision = token_safety::is_safe(&report, &policy, false);
            if json {
                println!("{}", serde_json::to_string_pretty(&(report, decision))?);
            } else {
                println!("Mint: {}", mint);
                println!("Program: {:?}", report.program_owner);
                println!("Decimals: {} Supply: {}", report.decimals, report.supply);
                println!("Decision: {}", if decision.safe {"SAFE"} else {"UNSAFE"});
                if !decision.reasons.is_empty() { println!("Reasons: {:?}", decision.reasons); }
                if !decision.warnings.is_empty() { println!("Warnings: {:?}", decision.warnings); }
            }
        }
        Commands::SimulateSell { pool_program, pool_account, mint_in, mint_out, amount, slippage_bps } => {
            let res = token_safety::sim::simulate_sell(
                &rpc,
                Pubkey::from_str(&pool_program)?,
                Pubkey::from_str(&pool_account)?,
                Pubkey::from_str(&mint_in)?,
                Pubkey::from_str(&mint_out)?,
                amount,
                slippage_bps,
            ).await?;
            println!("{}", serde_json::to_string_pretty(&res)?);
        }
    }
    Ok(())
}

