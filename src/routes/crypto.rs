use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;

use crate::{error::AppError, AppState};

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct CryptoHolding {
    pub id: i64,
    pub symbol: String,
    pub quantity: f64,
    pub price_usd: f64,
    pub value_usd: f64,
    pub risk_code: String,
}

#[derive(Template)]
#[template(path = "crypto.html")]
struct CryptoTemplate {
    total_crypto_value: f64,
    latest_as_of: String,
    holdings: Vec<CryptoHolding>,
    safe_value: f64,
    medium_value: f64,
    high_value: f64,
}

pub async fn index(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let latest_as_of: String = sqlx::query_scalar(
        "SELECT COALESCE(MAX(as_of), '') FROM snapshots",
    )
    .fetch_one(&state.pool)
    .await?;

    let total_crypto_value: f64 = if latest_as_of.is_empty() {
        0.0
    } else {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(s.value_usd) * 1.0, 0.0)
             FROM snapshots s
             JOIN assets a ON a.id = s.asset_id
             WHERE s.as_of = ?1 AND a.type_code = 'crypto'",
        )
        .bind(&latest_as_of)
        .fetch_one(&state.pool)
        .await?
    };

    let holdings: Vec<CryptoHolding> = if latest_as_of.is_empty() {
        vec![]
    } else {
        sqlx::query_as(
            "SELECT s.id, a.symbol, s.quantity,
                    COALESCE(s.price_usd, 0) AS price_usd,
                    s.value_usd,
                    COALESCE(a.risk_code, 'unknown') AS risk_code
             FROM snapshots s
             JOIN assets a ON a.id = s.asset_id
             WHERE s.as_of = ?1 AND a.type_code = 'crypto'
             ORDER BY s.value_usd DESC",
        )
        .bind(&latest_as_of)
        .fetch_all(&state.pool)
        .await?
    };

    let mut safe_value = 0.0_f64;
    let mut medium_value = 0.0_f64;
    let mut high_value = 0.0_f64;
    for h in &holdings {
        match h.risk_code.as_str() {
            "cat1_safe" => safe_value += h.value_usd,
            "cat2_medium" => medium_value += h.value_usd,
            "cat3_high" => high_value += h.value_usd,
            _ => {}
        }
    }

    Ok(CryptoTemplate {
        total_crypto_value,
        latest_as_of,
        holdings,
        safe_value,
        medium_value,
        high_value,
    })
}
