use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;

use crate::{error::AppError, AppState};

#[derive(Template)]
#[template(path = "flow.html")]
struct FlowTemplate {
    current_year_income: f64,
    current_year_expenses: f64,
    current_year_savings: f64,
    savings_rate_pct: f64,
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
    let year_start = format!("{current_year}-01-01");

    let income: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(salary_usd + bonus_usd - taxes_usd) * 1.0, 0.0) FROM income WHERE as_of >= ?1",
    )
    .bind(&year_start)
    .fetch_one(&state.pool)
    .await?;

    let expenses: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_usd) * 1.0, 0.0) FROM expenses WHERE as_of >= ?1",
    )
    .bind(&year_start)
    .fetch_one(&state.pool)
    .await?;

    let savings = income - expenses;
    let savings_rate_pct = if income > 0.0 {
        savings / income * 100.0
    } else {
        0.0
    };

    Ok(FlowTemplate {
        current_year_income: income,
        current_year_expenses: expenses,
        current_year_savings: savings,
        savings_rate_pct,
    })
}
