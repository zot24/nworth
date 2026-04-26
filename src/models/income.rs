use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct Income {
    pub id: i64,
    pub as_of: String,
    pub salary_usd: f64,
    pub per_year_usd: f64,
    pub bonus_usd: f64,
    pub taxes_usd: f64,
    pub company: Option<String>,
    pub source: Option<String>,
}
