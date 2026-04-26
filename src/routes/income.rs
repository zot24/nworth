use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;

use crate::{error::AppError, AppState};

#[derive(Debug)]
pub struct IncomeRow {
    pub id: i64,
    pub as_of: String,
    pub salary_usd: f64,
    pub bonus_usd: f64,
    pub taxes_usd: f64,
    pub company: String,
}

#[derive(Template)]
#[template(path = "income.html")]
struct IncomeTemplate {
    current_year_gross: f64,
    current_year_taxes: f64,
    current_year_net: f64,
    latest_company: String,
    rows: Vec<IncomeRow>,
}

pub async fn index(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let latest_as_of: String = sqlx::query_scalar(
        "SELECT COALESCE(MAX(as_of), '') FROM income",
    )
    .fetch_one(&state.pool)
    .await?;

    let current_year = if latest_as_of.len() >= 4 {
        &latest_as_of[..4]
    } else {
        "2026"
    };

    let ytd: (f64, f64) = sqlx::query_as(
        "SELECT COALESCE(SUM(salary_usd + bonus_usd) * 1.0, 0.0),
                COALESCE(SUM(taxes_usd) * 1.0, 0.0)
         FROM income WHERE as_of >= ?1",
    )
    .bind(format!("{current_year}-01-01"))
    .fetch_one(&state.pool)
    .await?;

    let latest_company: String = sqlx::query_scalar(
        "SELECT COALESCE(company, '—') FROM income WHERE as_of = ?1",
    )
    .bind(&latest_as_of)
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or_else(|| "—".to_string());

    let raw: Vec<(i64, String, f64, f64, f64, Option<String>)> = sqlx::query_as(
        "SELECT id, as_of, salary_usd, bonus_usd, taxes_usd, company
         FROM income ORDER BY as_of DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    let rows = raw
        .into_iter()
        .map(|(id, as_of, salary_usd, bonus_usd, taxes_usd, company)| IncomeRow {
            id,
            as_of,
            salary_usd,
            bonus_usd,
            taxes_usd,
            company: company.unwrap_or_else(|| "—".to_string()),
        })
        .collect();

    Ok(IncomeTemplate {
        current_year_gross: ytd.0,
        current_year_taxes: ytd.1,
        current_year_net: ytd.0 - ytd.1,
        latest_company,
        rows,
    })
}
