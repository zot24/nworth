use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct Snapshot {
    pub id: i64,
    pub as_of: String,
    pub account_id: i64,
    pub asset_id: i64,
    pub quantity: f64,
    pub price_usd: Option<f64>,
    pub value_usd: f64,
    pub source: Option<String>,
}

/// Aggregated net-worth point: total USD across all accounts/assets per date.
#[derive(Debug, Serialize, FromRow)]
pub struct NetWorthPoint {
    pub as_of: String,
    pub value_usd: f64,
}

/// Net worth broken out by asset type.
#[derive(Debug, Serialize, FromRow)]
pub struct NetWorthByType {
    pub as_of: String,
    pub type_code: String,
    pub value_usd: f64,
}
