//! /settings — app-level preferences.
//!
//! Phase 1 only stores + serves preferences. Templates still render USD; a
//! follow-up will route `display_currency` through the formatting helpers so
//! values render in the chosen currency.

use askama::Template;
use askama_axum::IntoResponse;
use axum::{extract::{Form, State}, response::Redirect};
use serde::Deserialize;

use crate::{error::AppError, AppState};

/// One per supported currency in the settings dropdown. `selected` is
/// pre-computed in the handler so the template doesn't have to compare
/// String to &String (askama limitation).
pub struct CurrencyOption {
    pub code: String,
    pub label: String,
    pub selected: bool,
}

#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsTemplate {
    display_currency: String,
    currency_options: Vec<CurrencyOption>,
}

pub async fn index(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let display_currency = get_setting(&state.pool, "display_currency").await?
        .unwrap_or_else(|| "USD".to_string());

    // Currencies offered: USD always + every currency we have any FX rate for.
    let mut codes: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT ccy FROM fx_rates ORDER BY ccy",
    ).fetch_all(&state.pool).await?;
    if !codes.iter().any(|c| c == "USD") { codes.insert(0, "USD".to_string()); }

    let currency_options: Vec<CurrencyOption> = codes.into_iter()
        .map(|code| CurrencyOption {
            selected: code == display_currency,
            label: currency_label(&code),
            code,
        })
        .collect();

    Ok(SettingsTemplate { display_currency, currency_options })
}

#[derive(Deserialize)]
pub struct SettingsForm {
    display_currency: String,
}

pub async fn save(
    State(state): State<AppState>,
    Form(f): Form<SettingsForm>,
) -> Result<Redirect, AppError> {
    let code = f.display_currency.trim().to_uppercase();
    if code.is_empty() {
        return Err(AppError::BadRequest("display_currency cannot be empty".into()));
    }
    set_setting(&state.pool, "display_currency", &code).await?;
    Ok(Redirect::to("/settings"))
}

// ────────── helpers ──────────

/// Friendly label for the dropdown. Keeps the list short by mapping codes we
/// use most frequently; everything else falls back to the bare code.
fn currency_label(code: &str) -> String {
    match code {
        "USD" => "USD — US Dollar".into(),
        "EUR" => "EUR — Euro".into(),
        "GBP" => "GBP — British Pound".into(),
        "PYG" => "PYG — Paraguayan Guaraní".into(),
        other => other.to_string(),
    }
}

pub(crate) async fn get_setting(pool: &sqlx::SqlitePool, key: &str) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
}

pub(crate) async fn set_setting(pool: &sqlx::SqlitePool, key: &str, value: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO settings(key, value, updated_at) VALUES(?1, ?2, CURRENT_TIMESTAMP)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}
