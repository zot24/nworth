use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;

use crate::{error::AppError, AppState};

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct StockPosition {
    pub symbol: String,
    pub account_name: String,
    pub quantity: f64,
    pub avg_cost: f64,
    pub value_usd: f64,
}

#[derive(Template)]
#[template(path = "stocks.html")]
struct StocksTemplate {
    total_stock_value: f64,
    total_dividends_ytd: f64,
    latest_as_of: String,
    positions: Vec<StockPosition>,
}

pub async fn index(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let latest_as_of: String = sqlx::query_scalar(
        "SELECT COALESCE(MAX(as_of), '') FROM snapshots",
    )
    .fetch_one(&state.pool)
    .await?;

    let total_stock_value: f64 = if latest_as_of.is_empty() {
        0.0
    } else {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(s.value_usd) * 1.0, 0.0)
             FROM snapshots s
             JOIN assets a ON a.id = s.asset_id
             WHERE s.as_of = ?1 AND a.type_code = 'stock'",
        )
        .bind(&latest_as_of)
        .fetch_one(&state.pool)
        .await?
    };

    let current_year = if latest_as_of.len() >= 4 { &latest_as_of[..4] } else { "2026" };
    let total_dividends_ytd: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(quantity) * 1.0, 0.0)
         FROM transactions
         WHERE kind = 'dividend' AND ts >= ?1",
    )
    .bind(format!("{current_year}-01-01"))
    .fetch_one(&state.pool)
    .await?;

    let positions: Vec<StockPosition> = sqlx::query_as(
        "SELECT a.symbol, ac.name AS account_name,
                p.quantity, COALESCE(p.avg_cost, 0) AS avg_cost,
                p.quantity * COALESCE(a.last_price, 0) AS value_usd
         FROM positions p
         JOIN accounts ac ON ac.id = p.account_id
         JOIN assets a ON a.id = p.asset_id
         WHERE a.type_code = 'stock' AND a.symbol != 'STOCKS_TOTAL'
         ORDER BY value_usd DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(StocksTemplate {
        total_stock_value,
        total_dividends_ytd,
        latest_as_of,
        positions,
    })
}
