use anyhow::{Result, Context};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, account::Account};
use common_types::QuickLiq;

/// Input information about pool and vaults.
#[derive(Clone)]
pub struct PoolInput {
    pub program: Pubkey,
    pub pool: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    pub decimals_a: u8,
    pub decimals_b: u8,
    pub vault_a: Option<Pubkey>,
    pub vault_b: Option<Pubkey>,
    pub sqrt_price_x64: Option<u128>,
    pub is_clmm: bool,
    pub quote_mints: Vec<Pubkey>,
}

/// Compute quick liquidity metrics.
pub fn compute_quick(
    rpc: &RpcClient,
    inp: &PoolInput,
) -> Result<QuickLiq> {
    let (reserves_a, reserves_b) = if let (Some(v_a), Some(v_b)) = (inp.vault_a, inp.vault_b) {
        let accs = rpc.get_multiple_accounts(&[v_a, v_b])?;
        (read_token_balance(accs.get(0)), read_token_balance(accs.get(1)))
    } else { (0u64, 0u64) };

    let price_ab = if inp.is_clmm {
        if let Some(sp) = inp.sqrt_price_x64 {
            let p = price_from_sqrtp_q64(sp, inp.decimals_a, inp.decimals_b);
            Some(p)
        } else { None }
    } else {
        if reserves_a > 0 && reserves_b > 0 {
            let adj = 10f64.powi((inp.decimals_a as i32) - (inp.decimals_b as i32));
            Some((reserves_b as f64 / reserves_a as f64) / adj)
        } else { None }
    };

    let (tvl_quote, qliq) = if let Some(is_a_quote) = is_quote(&inp.mint_a, &inp.mint_b, &inp.quote_mints) {
        let (dec_quote, dec_other, reserves_quote, reserves_other, price_other_in_quote) =
            if is_a_quote {
                (inp.decimals_a, inp.decimals_b, reserves_a, reserves_b, price_ab.map(|p| p.recip()))
            } else {
                (inp.decimals_b, inp.decimals_a, reserves_b, reserves_a, price_ab)
            };

        if let Some(p_oiq) = price_other_in_quote {
            let q = units_to_ui(reserves_quote, dec_quote);
            let o_ui = units_to_ui(reserves_other, dec_other);
            let other_in_quote = o_ui * p_oiq;
            let tvl = q + other_in_quote;
            let qliq = q.min(other_in_quote);
            (Some(tvl), Some(qliq))
        } else { (None, None) }
    } else { (None, None) };

    Ok(QuickLiq {
        price_ab,
        reserves_a,
        reserves_b,
        tvl_quote,
        quote_liquidity: qliq,
    })
}

fn read_token_balance(maybe_acc: Option<&Option<Account>>) -> u64 {
    if let Some(Some(acc)) = maybe_acc {
        let data = &acc.data;
        if data.len() >= 72 {
            let mut arr = [0u8;8];
            arr.copy_from_slice(&data[64..72]);
            return u64::from_le_bytes(arr);
        }
    }
    0
}

fn price_from_sqrtp_q64(sqrt_price_x64: u128, dec_a: u8, dec_b: u8) -> f64 {
    let sp = sqrt_price_x64 as f64;
    let p = (sp * sp) / (2f64.powi(128));
    let adj = 10f64.powi((dec_a as i32) - (dec_b as i32));
    p * adj
}

fn units_to_ui(amount: u64, decimals: u8) -> f64 {
    (amount as f64) / 10f64.powi(decimals as i32)
}

fn is_quote(a: &Pubkey, b: &Pubkey, quotes: &[Pubkey]) -> Option<bool> {
    if quotes.iter().any(|q| q == a) { return Some(true); }
    if quotes.iter().any(|q| q == b) { return Some(false); }
    None
}

