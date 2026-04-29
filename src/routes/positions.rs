use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;

use crate::{error::AppError, AppState};

#[derive(Debug)]
pub struct PositionRow {
    pub account_id: i64,
    pub asset_id: i64,
    pub account_name: String,
    pub symbol: String,
    pub type_code: String,
    pub quantity: f64,
    pub avg_cost: f64,
    pub last_price: f64,
    pub value_usd: f64,
    pub cost_basis: f64,
    pub market_value: f64,
    pub gain_loss: f64,
}

#[derive(Template)]
#[template(path = "positions.html")]
struct PositionsTemplate {
    positions: Vec<PositionRow>,
    total_invested: f64,
    total_current: f64,
}

pub async fn index(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    // last_price now lives on assets, computed value (qty × last_price) is
    // derived in SQL so the template doesn't need to know about the schema move.
    let rows: Vec<(i64, i64, String, String, String, f64, f64, f64, f64)> = sqlx::query_as(
        "SELECT p.account_id, p.asset_id, ac.name, a.symbol, a.type_code,
                p.quantity * 1.0, COALESCE(p.avg_cost * 1.0, 0.0), COALESCE(a.last_price * 1.0, 0.0),
                p.quantity * COALESCE(a.last_price, 0.0) AS value_usd
         FROM positions p
         JOIN accounts ac ON ac.id = p.account_id
         JOIN assets a ON a.id = p.asset_id
         ORDER BY value_usd DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    let positions: Vec<PositionRow> = rows
        .into_iter()
        .map(
            |(account_id, asset_id, account_name, symbol, type_code, quantity, avg_cost, last_price, _value_usd)| {
                let cost_basis = quantity * avg_cost;
                let market_value = quantity * last_price;
                let gain_loss = market_value - cost_basis;
                PositionRow {
                    account_id,
                    asset_id,
                    account_name,
                    symbol,
                    type_code,
                    quantity,
                    avg_cost,
                    last_price,
                    value_usd: market_value,
                    cost_basis,
                    market_value,
                    gain_loss,
                }
            },
        )
        .collect();

    let total_invested: f64 = positions.iter().map(|p| p.cost_basis).sum();
    let total_current: f64 = positions.iter().map(|p| p.market_value).sum();

    Ok(PositionsTemplate {
        positions,
        total_invested,
        total_current,
    })
}
