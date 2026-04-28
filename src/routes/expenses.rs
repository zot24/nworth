use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Query, State};
use serde::Deserialize;

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

#[derive(Deserialize)]
pub struct ExpensesQuery {
    /// Selected year filter — "all" or "YYYY". Defaults to "all".
    pub year: Option<String>,
}

/// One per pickable year; carries the selection flag so the template doesn't
/// have to do `selected_year == y` (askama doesn't support String == &String
/// or `*` deref in expressions).
pub struct YearOption {
    pub year: String,
    pub selected: bool,
}

#[derive(Template)]
#[template(path = "expenses.html")]
struct ExpensesTemplate {
    all_selected: bool,
    year_options: Vec<YearOption>,
    period_total: f64,        // sum over the selected period
    period_avg: f64,          // monthly average over the selected period
    period_max: f64,          // largest single month in the period
    period_label: String,     // "2024" or "all years"
    latest_as_of: String,
    rows: Vec<ExpenseRow>,
}

pub async fn index(
    State(state): State<AppState>,
    Query(q): Query<ExpensesQuery>,
) -> Result<impl IntoResponse, AppError> {
    let latest_as_of: String = sqlx::query_scalar(
        "SELECT COALESCE(MAX(as_of), '') FROM expenses",
    ).fetch_one(&state.pool).await?;

    // Build distinct year list (descending) for the picker.
    let available_years: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT SUBSTR(as_of, 1, 4) AS y FROM expenses ORDER BY y DESC",
    ).fetch_all(&state.pool).await?;

    // Resolve selected year: must be one of the available ones, otherwise "all".
    let selected_year = match q.year.as_deref() {
        Some(y) if y == "all" => "all".to_string(),
        Some(y) if available_years.iter().any(|av| av == y) => y.to_string(),
        _ => "all".to_string(),
    };

    // Period bounds (inclusive). For "all" we leave the range wide open.
    let (period_lo, period_hi) = if selected_year == "all" {
        ("0000-01-01".to_string(), "9999-12-31".to_string())
    } else {
        (format!("{selected_year}-01-01"), format!("{selected_year}-12-31"))
    };

    // Period stats — total, monthly average, max single month.
    let (period_total, n_months, period_max): (f64, i64, f64) = sqlx::query_as(
        "SELECT COALESCE(SUM(amount_usd) * 1.0, 0.0),
                COUNT(*),
                COALESCE(MAX(amount_usd) * 1.0, 0.0)
         FROM expenses WHERE as_of >= ?1 AND as_of <= ?2",
    ).bind(&period_lo).bind(&period_hi).fetch_one(&state.pool).await?;

    let period_avg = if n_months > 0 { period_total / (n_months as f64) } else { 0.0 };
    let period_label = if selected_year == "all" { "all years".to_string() } else { selected_year.clone() };

    let raw: Vec<(i64, String, f64, Option<String>, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT e.id, e.as_of, e.amount_usd * 1.0, e.place, c.name, c.color,
                GROUP_CONCAT(l.name, ', ')
         FROM expenses e
         LEFT JOIN categories c ON c.id = e.category_id
         LEFT JOIN expense_labels el ON el.expense_id = e.id
         LEFT JOIN labels l ON l.id = el.label_id
         WHERE e.as_of >= ?1 AND e.as_of <= ?2
         GROUP BY e.id ORDER BY e.as_of DESC",
    ).bind(&period_lo).bind(&period_hi).fetch_all(&state.pool).await?;

    let rows = raw.into_iter()
        .map(|(id, as_of, amount_usd, place, cat_name, cat_color, labels)| ExpenseRow {
            id, as_of, amount_usd,
            place: place.unwrap_or_else(|| "—".to_string()),
            category_name: cat_name.unwrap_or_default(),
            category_color: cat_color.unwrap_or_default(),
            labels_display: labels.unwrap_or_default(),
        })
        .collect();

    let all_selected = selected_year == "all";
    let year_options: Vec<YearOption> = available_years.into_iter()
        .map(|year| YearOption { selected: year == selected_year, year })
        .collect();

    Ok(ExpensesTemplate {
        all_selected, year_options,
        period_total, period_avg, period_max, period_label,
        latest_as_of, rows,
    })
}
