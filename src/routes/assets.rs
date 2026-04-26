use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;

use crate::{error::AppError, models::asset::Asset, AppState};

#[derive(Template)]
#[template(path = "assets.html")]
struct AssetsTemplate {
    assets: Vec<Asset>,
}

pub async fn list(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let assets = sqlx::query_as::<_, Asset>(
        "SELECT id, symbol, name, type_code, chain_code, risk_code,
                coingecko_id, yahoo_ticker, target_pct, is_stable, active
         FROM assets
         WHERE active = 1
         ORDER BY type_code, symbol",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(AssetsTemplate { assets })
}
