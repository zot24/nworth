use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;

use crate::{error::AppError, AppState};

#[derive(Debug)]
pub struct ExpenseRow {
    pub id: i64,
    pub as_of: String,
    pub amount_usd: f64,
    pub place: String,
    pub category_name: String,
    pub category_color: String,
    pub labels_display: String,
}

#[derive(Template)]
#[template(path = "expenses.html")]
struct ExpensesTemplate {
    current_year_total: f64,
    current_month_amount: f64,
    latest_as_of: String,
    rows: Vec<ExpenseRow>,
}

pub async fn index(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let latest_as_of: String = sqlx::query_scalar(
        "SELECT COALESCE(MAX(as_of), '') FROM expenses",
    ).fetch_one(&state.pool).await?;

    let current_year = if latest_as_of.len() >= 4 { &latest_as_of[..4] } else { "2026" };

    let current_year_total: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_usd) * 1.0, 0.0) FROM expenses WHERE as_of >= ?1",
    ).bind(format!("{current_year}-01-01")).fetch_one(&state.pool).await?;

    let current_month_amount: f64 = if latest_as_of.is_empty() { 0.0 } else {
        sqlx::query_scalar("SELECT COALESCE(amount_usd, 0) FROM expenses WHERE as_of = ?1")
            .bind(&latest_as_of).fetch_one(&state.pool).await.unwrap_or(0.0)
    };

    // One query joining category + concatenated labels
    let raw: Vec<(i64, String, f64, Option<String>, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT e.id, e.as_of, e.amount_usd * 1.0, e.place, c.name, c.color,
                GROUP_CONCAT(l.name, ', ')
         FROM expenses e
         LEFT JOIN categories c ON c.id = e.category_id
         LEFT JOIN expense_labels el ON el.expense_id = e.id
         LEFT JOIN labels l ON l.id = el.label_id
         GROUP BY e.id ORDER BY e.as_of DESC",
    ).fetch_all(&state.pool).await?;

    let rows = raw.into_iter()
        .map(|(id, as_of, amount_usd, place, cat_name, cat_color, labels)| ExpenseRow {
            id, as_of, amount_usd,
            place: place.unwrap_or_else(|| "—".to_string()),
            category_name: cat_name.unwrap_or_default(),
            category_color: cat_color.unwrap_or_default(),
            labels_display: labels.unwrap_or_default(),
        })
        .collect();

    Ok(ExpensesTemplate { current_year_total, current_month_amount, latest_as_of, rows })
}
