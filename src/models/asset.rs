use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct Asset {
    pub id: i64,
    pub symbol: String,
    pub name: Option<String>,
    pub type_code: String,
    pub chain_code: Option<String>,
    pub risk_code: Option<String>,
    pub coingecko_id: Option<String>,
    pub yahoo_ticker: Option<String>,
    pub active: i64,
}
