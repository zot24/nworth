use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct Expense {
    pub id: i64,
    pub as_of: String,
    pub amount_usd: f64,
    pub place: Option<String>,
    pub notes: Option<String>,
    pub source: Option<String>,
}
