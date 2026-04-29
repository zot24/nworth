//! Price feed binary — fetches crypto, stock, and FX prices, caches them in SQLite.
//!
//! Usage:
//!   cargo run --bin nworth-feed -- --once           # fetch once and exit
//!   cargo run --bin nworth-feed -- --loop 3600       # fetch every 3600 seconds

use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;

use portfolio_tracker::services::{fx, prices};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,pricefeed=debug".parse().unwrap()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("--once");
    let interval_secs: u64 = if mode == "--loop" {
        args.get(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(3600)
    } else {
        0
    };

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://data/portfolio.db?mode=rwc".to_string());
    let pool = SqlitePool::connect(&db_url).await?;
    let cg_key = std::env::var("COINGECKO_API_KEY").ok();

    tracing::info!("nworth-feed starting (mode={mode}, db={db_url})");

    loop {
        if let Err(e) = run_cycle(&pool, cg_key.as_deref()).await {
            tracing::error!("price fetch cycle failed: {e:#}");
        }

        if mode != "--loop" {
            break;
        }
        tracing::info!("sleeping {interval_secs}s until next cycle");
        tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
    }

    Ok(())
}

async fn run_cycle(pool: &SqlitePool, cg_key: Option<&str>) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let today = Utc::now().format("%Y-%m-%d").to_string();

    // --- 1. Crypto prices via CoinGecko ---
    let crypto_assets: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, coingecko_id FROM assets WHERE coingecko_id IS NOT NULL AND active = 1",
    )
    .fetch_all(pool)
    .await?;

    if !crypto_assets.is_empty() {
        let ids: Vec<&str> = crypto_assets.iter().map(|(_, cg)| cg.as_str()).collect();
        tracing::info!("fetching {} crypto prices from CoinGecko", ids.len());

        match prices::coingecko_spot(&client, cg_key, &ids).await {
            Ok(price_map) => {
                let mut count = 0;
                for (asset_id, cg_id) in &crypto_assets {
                    if let Some(price) = price_map.get(cg_id) {
                        prices::record_price(pool, *asset_id, &today, *price, "coingecko")
                            .await?;
                        count += 1;
                    }
                }
                tracing::info!("recorded {count} crypto prices");
            }
            Err(e) => tracing::warn!("CoinGecko fetch failed: {e:#}"),
        }
    }

    // --- 2. Stock prices via Yahoo Finance ---
    let stock_assets: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, yahoo_ticker FROM assets WHERE yahoo_ticker IS NOT NULL AND active = 1",
    )
    .fetch_all(pool)
    .await?;

    if !stock_assets.is_empty() {
        tracing::info!("fetching {} stock prices from Yahoo Finance", stock_assets.len());
        let mut count = 0;
        for (asset_id, ticker) in &stock_assets {
            match prices::yahoo_quote(&client, ticker).await {
                Ok(Some(price)) => {
                    prices::record_price(pool, *asset_id, &today, price, "yahoo").await?;
                    count += 1;
                }
                Ok(None) => tracing::warn!("no price returned for {ticker}"),
                Err(e) => tracing::warn!("Yahoo fetch failed for {ticker}: {e:#}"),
            }
            // Small delay to avoid rate limiting
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        tracing::info!("recorded {count} stock prices");
    }

    // --- 3. FX rates ---
    let ccys = ["EUR", "GBP", "PYG"];
    tracing::info!("fetching FX rates for {:?}", ccys);
    match fx::latest_usd_rates(&client, &ccys).await {
        Ok(rates) => {
            for (ccy, rate) in &rates {
                fx::record_fx(pool, ccy, &today, *rate).await?;
            }
            tracing::info!("recorded {} FX rates", rates.len());
        }
        Err(e) => tracing::warn!("FX fetch failed: {e:#}"),
    }

    // --- 4. Update assets.last_price from latest price_history per asset ---
    // One row written per asset (not per position), so BTC at Robinhood +
    // Gemini + Ledger results in one update, not three. Read paths join
    // positions × assets to compute live values on demand.
    update_asset_prices(pool).await?;

    // --- 5. Create / update the *current month's* snapshot ---
    // The snapshot's as_of is anchored to the first day of the current month,
    // so each cycle re-runs the same row (UNIQUE(as_of, account_id, asset_id)
    // + ON CONFLICT DO UPDATE). Net effect: one snapshot per (month, account,
    // asset), continuously refreshed with live prices throughout the month
    // and naturally "frozen" once the next month starts and a new anchor row
    // is created. No daily-row accumulation, no separate prune step needed.
    let month_anchor = Utc::now().format("%Y-%m-01").to_string();
    create_snapshots(pool, &month_anchor).await?;

    tracing::info!("price fetch cycle complete");
    Ok(())
}

/// Refreshes assets.last_price + assets.last_price_as_of from the most recent
/// price_history row per asset. One row updated per asset, not per position.
async fn update_asset_prices(pool: &SqlitePool) -> Result<()> {
    let updated = sqlx::query(
        "UPDATE assets SET
            last_price = (
                SELECT ph.price_usd FROM price_history ph
                WHERE ph.asset_id = assets.id
                ORDER BY ph.as_of DESC LIMIT 1
            ),
            last_price_as_of = (
                SELECT ph.as_of FROM price_history ph
                WHERE ph.asset_id = assets.id
                ORDER BY ph.as_of DESC LIMIT 1
            )
        WHERE EXISTS (
            SELECT 1 FROM price_history ph
            WHERE ph.asset_id = assets.id
        )",
    )
    .execute(pool)
    .await?;

    tracing::info!("updated {} asset prices from price_history", updated.rows_affected());
    Ok(())
}

/// Creates / updates the current month's snapshot rows by joining positions
/// to assets to read each asset's live last_price. value_usd is computed at
/// write time as quantity × last_price.
async fn create_snapshots(pool: &SqlitePool, anchor: &str) -> Result<()> {
    let inserted = sqlx::query(
        "INSERT INTO snapshots(as_of, account_id, asset_id, quantity, price_usd, value_usd, source)
         SELECT ?1, p.account_id, p.asset_id, p.quantity, a.last_price,
                p.quantity * COALESCE(a.last_price, 0), 'pricefeed'
         FROM positions p
         JOIN assets a ON a.id = p.asset_id
         WHERE a.last_price > 0
         ON CONFLICT(as_of, account_id, asset_id) DO UPDATE SET
           quantity  = excluded.quantity,
           price_usd = excluded.price_usd,
           value_usd = excluded.value_usd,
           source    = excluded.source",
    )
    .bind(anchor)
    .execute(pool)
    .await?;

    tracing::info!("created/updated {} snapshots for {}", inserted.rows_affected(), anchor);
    Ok(())
}
