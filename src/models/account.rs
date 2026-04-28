use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub type_code: String,
    pub institution: Option<String>,
    pub chain_code: Option<String>,
    pub active: i64,
    pub notes: Option<String>,
    /// Three-way classification: 'investment' (counts in drift), 'operating'
    /// (day-to-day cash, net-worth-only), 'property' (physical things, net-worth-only).
    pub role: String,
}
