use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;

use crate::{error::AppError, AppState};

/// Grouped view: one row per category with bull/crab/bear columns
#[derive(Debug)]
pub struct TargetGroup {
    pub category: String,
    pub bull_pct: f64,
    pub bull_id: i64,
    pub crab_pct: f64,
    pub crab_id: i64,
    pub bear_pct: f64,
    pub bear_id: i64,
}

#[derive(Template)]
#[template(path = "targets.html")]
struct TargetsTemplate {
    groups: Vec<TargetGroup>,
}

pub async fn index(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let raw: Vec<(i64, String, String, f64, Option<String>)> = sqlx::query_as(
        "SELECT id, category, market_mode, target_pct, notes
         FROM allocation_targets ORDER BY category, market_mode",
    )
    .fetch_all(&state.pool)
    .await?;

    // Group by category for the 3-column view
    let mut cat_map: std::collections::BTreeMap<String, (f64, i64, f64, i64, f64, i64)> =
        std::collections::BTreeMap::new();
    for (id, category, mode, pct, _) in &raw {
        let entry = cat_map.entry(category.clone()).or_insert((0.0, 0, 0.0, 0, 0.0, 0));
        match mode.as_str() {
            "bull" => { entry.0 = *pct; entry.1 = *id; }
            "crab" => { entry.2 = *pct; entry.3 = *id; }
            "bear" => { entry.4 = *pct; entry.5 = *id; }
            _ => {}
        }
    }

    let groups: Vec<TargetGroup> = cat_map
        .into_iter()
        .map(|(category, (bull_pct, bull_id, crab_pct, crab_id, bear_pct, bear_id))| TargetGroup {
            category,
            bull_pct, bull_id,
            crab_pct, crab_id,
            bear_pct, bear_id,
        })
        .collect();

    Ok(TargetsTemplate { groups })
}
