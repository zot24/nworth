use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;

use crate::{error::AppError, AppState};

#[derive(Debug)]
pub struct CashBalance {
    pub id: i64,
    pub account_name: String,
    pub symbol: String,
    pub quantity: f64,
    pub value_usd: f64,
}

#[derive(Template)]
#[template(path = "cash.html")]
struct CashTemplate {
    total_fiat_usd: f64,
    total_stables_usd: f64,
    total_combined: f64,
    latest_as_of: String,
    balances: Vec<CashBalance>,
    currency_labels: String,
    currency_values: String,
}

pub async fn index(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let latest_as_of: String = sqlx::query_scalar(
        "SELECT COALESCE(MAX(as_of), '') FROM snapshots",
    )
    .fetch_one(&state.pool)
    .await?;

    if latest_as_of.is_empty() {
        return Ok(CashTemplate {
            total_fiat_usd: 0.0,
            total_stables_usd: 0.0,
            total_combined: 0.0,
            latest_as_of,
            balances: vec![],
            currency_labels: "[]".to_string(),
            currency_values: "[]".to_string(),
        });
    }

    let total_fiat_usd: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(s.value_usd) * 1.0, 0.0)
         FROM snapshots s
         JOIN assets a ON a.id = s.asset_id
         WHERE s.as_of = ?1 AND a.type_code = 'fiat'",
    )
    .bind(&latest_as_of)
    .fetch_one(&state.pool)
    .await?;

    let total_stables_usd: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(s.value_usd) * 1.0, 0.0)
         FROM snapshots s
         JOIN assets a ON a.id = s.asset_id
         WHERE s.as_of = ?1 AND a.type_code = 'stable'",
    )
    .bind(&latest_as_of)
    .fetch_one(&state.pool)
    .await?;

    let rows: Vec<(i64, String, String, f64, f64)> = sqlx::query_as(
        "SELECT s.id, ac.name, a.symbol, s.quantity * 1.0, s.value_usd * 1.0
         FROM snapshots s
         JOIN accounts ac ON ac.id = s.account_id
         JOIN assets a ON a.id = s.asset_id
         WHERE s.as_of = ?1 AND a.type_code IN ('fiat', 'stable')
         ORDER BY s.value_usd DESC",
    )
    .bind(&latest_as_of)
    .fetch_all(&state.pool)
    .await?;

    let balances: Vec<CashBalance> = rows
        .iter()
        .map(|(id, account_name, symbol, quantity, value_usd)| CashBalance {
            id: *id,
            account_name: account_name.clone(),
            symbol: symbol.clone(),
            quantity: *quantity,
            value_usd: *value_usd,
        })
        .collect();

    // Currency breakdown for doughnut chart
    let ccy_rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT a.symbol, SUM(s.value_usd) * 1.0
         FROM snapshots s
         JOIN assets a ON a.id = s.asset_id
         WHERE s.as_of = ?1 AND a.type_code IN ('fiat', 'stable')
         GROUP BY a.symbol
         HAVING SUM(s.value_usd) > 0
         ORDER BY 2 DESC",
    )
    .bind(&latest_as_of)
    .fetch_all(&state.pool)
    .await?;

    let currency_labels = format!(
        "[{}]",
        ccy_rows
            .iter()
            .map(|(s, _)| format!("\"{}\"", s))
            .collect::<Vec<_>>()
            .join(",")
    );
    let currency_values = format!(
        "[{}]",
        ccy_rows
            .iter()
            .map(|(_, v)| format!("{:.2}", v))
            .collect::<Vec<_>>()
            .join(",")
    );

    Ok(CashTemplate {
        total_fiat_usd,
        total_stables_usd,
        total_combined: total_fiat_usd + total_stables_usd,
        latest_as_of,
        balances,
        currency_labels,
        currency_values,
    })
}
