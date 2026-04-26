//! Price pullers. CoinGecko for crypto, Yahoo Finance for stocks.
//! Results written to `price_history`.

use anyhow::Result;
use serde::Deserialize;
use sqlx::SqlitePool;

const CG_BASE: &str = "https://api.coingecko.com/api/v3";

#[derive(Debug, Deserialize)]
struct CoinGeckoSimpleResp(std::collections::HashMap<String, CgPrice>);

#[derive(Debug, Deserialize)]
struct CgPrice {
    usd: f64,
}

/// Fetch spot USD prices for a list of CoinGecko IDs (e.g., ["bitcoin","solana"]).
pub async fn coingecko_spot(
    client: &reqwest::Client,
    api_key: Option<&str>,
    ids: &[&str],
) -> Result<std::collections::HashMap<String, f64>> {
    if ids.is_empty() {
        return Ok(Default::default());
    }
    let url = format!("{}/simple/price", CG_BASE);
    let mut req = client.get(url).query(&[
        ("ids", ids.join(",")),
        ("vs_currencies", "usd".into()),
    ]);
    if let Some(k) = api_key {
        req = req.header("x-cg-demo-api-key", k);
    }
    let resp: CoinGeckoSimpleResp = req.send().await?.error_for_status()?.json().await?;
    Ok(resp.0.into_iter().map(|(k, v)| (k, v.usd)).collect())
}

/// Fetch current quote for a stock ticker from Yahoo's unofficial chart endpoint.
/// (Stub — wire up when we have the full ticker list. For production, use Alpaca.)
pub async fn yahoo_quote(client: &reqwest::Client, ticker: &str) -> Result<Option<f64>> {
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?range=1d&interval=1d",
        ticker
    );
    let resp: serde_json::Value = client.get(url).send().await?.error_for_status()?.json().await?;
    let price = resp
        .pointer("/chart/result/0/meta/regularMarketPrice")
        .and_then(|v| v.as_f64());
    Ok(price)
}

/// Upsert a price point into price_history.
pub async fn record_price(
    pool: &SqlitePool,
    asset_id: i64,
    as_of: &str,
    price_usd: f64,
    source: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO price_history(asset_id, as_of, price_usd, source)
         VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(asset_id, as_of) DO UPDATE SET
            price_usd = excluded.price_usd,
            source    = excluded.source",
    )
    .bind(asset_id)
    .bind(as_of)
    .bind(price_usd)
    .bind(source)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_coingecko_simple_price_shape() {
        let sample = r#"{
            "bitcoin":  {"usd": 75757.0},
            "solana":   {"usd": 85.41},
            "ethereum": {"usd": 2400.5}
        }"#;
        let parsed: CoinGeckoSimpleResp = serde_json::from_str(sample).unwrap();
        let map: std::collections::HashMap<_, _> =
            parsed.0.into_iter().map(|(k, v)| (k, v.usd)).collect();
        assert_eq!(map.get("bitcoin"), Some(&75757.0));
        assert_eq!(map.get("solana"), Some(&85.41));
        assert_eq!(map.get("ethereum"), Some(&2400.5));
    }

    #[test]
    fn yahoo_quote_path_extracts_price() {
        // Mimics the yfinance chart response shape we extract.
        let body: serde_json::Value = serde_json::json!({
            "chart": {"result": [{"meta": {"regularMarketPrice": 512.34}}]}
        });
        let price = body
            .pointer("/chart/result/0/meta/regularMarketPrice")
            .and_then(|v| v.as_f64());
        assert_eq!(price, Some(512.34));
    }

    #[tokio::test]
    async fn record_price_upserts() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        // need an asset to reference
        let asset_id: i64 = sqlx::query_scalar(
            "INSERT INTO assets(symbol, type_code, active) VALUES('TEST','crypto',1) RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        record_price(&pool, asset_id, "2026-04-01", 100.0, "test").await.unwrap();
        record_price(&pool, asset_id, "2026-04-01", 150.0, "test").await.unwrap(); // upsert

        let (price,): (f64,) = sqlx::query_as(
            "SELECT price_usd FROM price_history WHERE asset_id = ? AND as_of = ?",
        )
        .bind(asset_id)
        .bind("2026-04-01")
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(price, 150.0, "upsert should overwrite price");
    }
}
