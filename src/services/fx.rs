//! FX rates via exchangerate.host (free, USD-based).

use anyhow::Result;
use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(Debug, Deserialize)]
struct ExRateResp {
    rates: std::collections::HashMap<String, f64>,
}

/// Fetches latest rates for given currencies (inverted to "1 ccy = X USD").
pub async fn latest_usd_rates(
    client: &reqwest::Client,
    ccys: &[&str],
) -> Result<std::collections::HashMap<String, f64>> {
    let url = "https://api.exchangerate.host/latest?base=USD";
    let resp: ExRateResp = client.get(url).send().await?.error_for_status()?.json().await?;
    let mut out = std::collections::HashMap::new();
    for c in ccys {
        if let Some(r) = resp.rates.get(*c) {
            // invert: stored as "1 ccy = X USD"
            if *r > 0.0 {
                out.insert(c.to_string(), 1.0 / r);
            }
        }
    }
    Ok(out)
}

pub async fn record_fx(
    pool: &SqlitePool,
    ccy: &str,
    as_of: &str,
    rate_usd: f64,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO fx_rates(ccy, as_of, rate_usd, source)
         VALUES(?1, ?2, ?3, 'exchangerate.host')
         ON CONFLICT(ccy, as_of) DO UPDATE SET rate_usd = excluded.rate_usd",
    )
    .bind(ccy)
    .bind(as_of)
    .bind(rate_usd)
    .execute(pool)
    .await?;
    Ok(())
}
