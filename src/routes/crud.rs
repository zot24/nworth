//! CRUD endpoints for accounts, assets, income, expenses, and snapshots.
//! All use HTML form POST → redirect pattern (no JSON, no HTMX partials).

use axum::{
    extract::{Form, Path, State},
    response::Redirect,
};
use serde::Deserialize;

use chrono::Utc;
use crate::{error::AppError, AppState};

// ────────── Accounts ──────────

#[derive(Deserialize)]
pub struct AccountForm {
    name: String,
    type_code: String,
    institution: Option<String>,
    chain_code: Option<String>,
    notes: Option<String>,
    /// Checkbox value — present when checked ("on"/"true"/"1"), absent when unchecked.
    is_investment: Option<String>,
}

fn investment_flag(raw: &Option<String>) -> i64 {
    match raw.as_deref() {
        // Unchecked checkbox = not present in form data → default true (investment).
        // Explicit "0" / "false" → owned. Anything else (including "on") → investment.
        Some("0") | Some("false") => 0,
        _ => 1,
    }
}

/// Treat empty form fields as NULL. Important for `chain_code` (FK to chains(code))
/// where an empty string would violate the constraint.
fn nullable(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|v| !v.is_empty())
}

pub async fn create_account(
    State(state): State<AppState>,
    Form(f): Form<AccountForm>,
) -> Result<Redirect, AppError> {
    let is_investment = investment_flag(&f.is_investment);
    sqlx::query(
        "INSERT INTO accounts(name, type_code, institution, chain_code, active, notes, is_investment)
         VALUES(?1, ?2, ?3, ?4, 1, ?5, ?6)",
    )
    .bind(&f.name)
    .bind(&f.type_code)
    .bind(nullable(&f.institution))
    .bind(nullable(&f.chain_code))
    .bind(nullable(&f.notes))
    .bind(is_investment)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to("/data?tab=accounts"))
}

pub async fn update_account(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(f): Form<AccountForm>,
) -> Result<Redirect, AppError> {
    let is_investment = investment_flag(&f.is_investment);
    sqlx::query(
        "UPDATE accounts SET name=?1, type_code=?2, institution=?3, chain_code=?4, notes=?5, is_investment=?6
         WHERE id=?7",
    )
    .bind(&f.name)
    .bind(&f.type_code)
    .bind(nullable(&f.institution))
    .bind(nullable(&f.chain_code))
    .bind(nullable(&f.notes))
    .bind(is_investment)
    .bind(id)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to("/data?tab=accounts"))
}

pub async fn delete_account(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Redirect, AppError> {
    smart_delete_account(&state.pool, id).await?;
    Ok(Redirect::to(q.get("redirect").map(|s| s.as_str()).unwrap_or("/accounts")))
}

/// Hard-delete the account if nothing references it (snapshots, positions, transactions);
/// otherwise mark inactive to preserve historical data integrity.
pub(crate) async fn smart_delete_account(pool: &sqlx::SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
    let refs: i64 = sqlx::query_scalar(
        "SELECT
            (SELECT COUNT(*) FROM snapshots WHERE account_id = ?1)
          + (SELECT COUNT(*) FROM positions WHERE account_id = ?1)
          + (SELECT COUNT(*) FROM transactions WHERE account_id = ?1 OR counterparty_account_id = ?1)",
    ).bind(id).fetch_one(pool).await?;
    if refs == 0 {
        sqlx::query("DELETE FROM accounts WHERE id = ?1").bind(id).execute(pool).await?;
        Ok(true)
    } else {
        sqlx::query("UPDATE accounts SET active = 0 WHERE id = ?1").bind(id).execute(pool).await?;
        Ok(false)
    }
}

// Owned-asset value updates and creation now go through the regular /data
// editor: type-`owned` assets are managed in /data?tab=assets like any other
// asset, and their valuations are inserted/updated via /data?tab=snapshots.
// Keeping a single edit surface avoids the "wait, where do I do this?" problem
// that came from having a parallel quick-form on /accounts.

// ────────── Assets ──────────

#[derive(Deserialize)]
pub struct AssetForm {
    symbol: String,
    name: Option<String>,
    type_code: String,
    chain_code: Option<String>,
    risk_code: Option<String>,
    coingecko_id: Option<String>,
    yahoo_ticker: Option<String>,
    target_pct: Option<f64>,
    is_stable: Option<String>, // "on" or missing
}

pub async fn create_asset(
    State(state): State<AppState>,
    Form(f): Form<AssetForm>,
) -> Result<Redirect, AppError> {
    let is_stable: i64 = if f.is_stable.is_some() { 1 } else { 0 };
    sqlx::query(
        "INSERT INTO assets(symbol, name, type_code, chain_code, risk_code,
                             coingecko_id, yahoo_ticker, target_pct, is_stable, active)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,1)",
    )
    .bind(&f.symbol)
    .bind(nullable(&f.name))
    .bind(&f.type_code)
    .bind(nullable(&f.chain_code))
    .bind(nullable(&f.risk_code))
    .bind(nullable(&f.coingecko_id))
    .bind(nullable(&f.yahoo_ticker))
    .bind(f.target_pct)
    .bind(is_stable)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to("/data?tab=assets"))
}

pub async fn update_asset(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(f): Form<AssetForm>,
) -> Result<Redirect, AppError> {
    let is_stable: i64 = if f.is_stable.is_some() { 1 } else { 0 };
    sqlx::query(
        "UPDATE assets SET symbol=?1, name=?2, type_code=?3, chain_code=?4, risk_code=?5,
                coingecko_id=?6, yahoo_ticker=?7, target_pct=?8, is_stable=?9
         WHERE id=?10",
    )
    .bind(&f.symbol)
    .bind(nullable(&f.name))
    .bind(&f.type_code)
    .bind(nullable(&f.chain_code))
    .bind(nullable(&f.risk_code))
    .bind(nullable(&f.coingecko_id))
    .bind(nullable(&f.yahoo_ticker))
    .bind(f.target_pct)
    .bind(is_stable)
    .bind(id)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to("/data?tab=assets"))
}

pub async fn delete_asset(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Redirect, AppError> {
    smart_delete_asset(&state.pool, id).await?;
    Ok(Redirect::to(q.get("redirect").map(|s| s.as_str()).unwrap_or("/assets")))
}

/// Hard-delete the asset if nothing references it (snapshots, positions, transactions);
/// otherwise mark inactive. price_history is intentionally ignored — it's market data, not user data.
pub(crate) async fn smart_delete_asset(pool: &sqlx::SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
    let refs: i64 = sqlx::query_scalar(
        "SELECT
            (SELECT COUNT(*) FROM snapshots WHERE asset_id = ?1)
          + (SELECT COUNT(*) FROM positions WHERE asset_id = ?1)
          + (SELECT COUNT(*) FROM transactions WHERE asset_id = ?1)",
    ).bind(id).fetch_one(pool).await?;
    if refs == 0 {
        sqlx::query("DELETE FROM assets WHERE id = ?1").bind(id).execute(pool).await?;
        Ok(true)
    } else {
        sqlx::query("UPDATE assets SET active = 0 WHERE id = ?1").bind(id).execute(pool).await?;
        Ok(false)
    }
}

// ────────── Income ──────────

#[derive(Deserialize)]
pub struct IncomeForm {
    as_of: String,
    salary_usd: f64,
    bonus_usd: Option<f64>,
    taxes_usd: Option<f64>,
    company: Option<String>,
}

pub async fn create_income(
    State(state): State<AppState>,
    Form(f): Form<IncomeForm>,
) -> Result<Redirect, AppError> {
    sqlx::query(
        "INSERT INTO income(as_of, salary_usd, per_year_usd, bonus_usd, taxes_usd, company, source)
         VALUES(?1, ?2, 0, ?3, ?4, ?5, 'manual')
         ON CONFLICT(as_of) DO UPDATE SET
           salary_usd = excluded.salary_usd,
           bonus_usd  = excluded.bonus_usd,
           taxes_usd  = excluded.taxes_usd,
           company    = excluded.company",
    )
    .bind(&f.as_of)
    .bind(f.salary_usd)
    .bind(f.bonus_usd.unwrap_or(0.0))
    .bind(f.taxes_usd.unwrap_or(0.0))
    .bind(&f.company)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to("/income"))
}

pub async fn delete_income(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError> {
    sqlx::query("DELETE FROM income WHERE id = ?1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(Redirect::to("/income"))
}

// ────────── Expenses ──────────

#[derive(Deserialize)]
pub struct ExpenseForm {
    as_of: String,
    amount_usd: f64,
    place: Option<String>,
    category_id: Option<i64>,
    /// Comma-separated list of label IDs from a multi-select hidden input.
    label_ids: Option<String>,
    redirect: Option<String>,
}

pub async fn create_expense(
    State(state): State<AppState>,
    Form(f): Form<ExpenseForm>,
) -> Result<Redirect, AppError> {
    let category_id = f.category_id.filter(|id| *id > 0);
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO expenses(as_of, amount_usd, place, source, category_id)
         VALUES(?1, ?2, ?3, 'manual', ?4)
         ON CONFLICT(as_of) DO UPDATE SET
           amount_usd  = excluded.amount_usd,
           place       = excluded.place,
           category_id = excluded.category_id
         RETURNING id",
    )
    .bind(&f.as_of)
    .bind(f.amount_usd)
    .bind(&f.place)
    .bind(category_id)
    .fetch_one(&state.pool)
    .await?;

    if let Some(raw) = f.label_ids.as_deref() {
        sqlx::query("DELETE FROM expense_labels WHERE expense_id = ?1").bind(id).execute(&state.pool).await?;
        for s in raw.split(',').filter(|s| !s.trim().is_empty()) {
            if let Ok(lid) = s.trim().parse::<i64>() {
                sqlx::query("INSERT OR IGNORE INTO expense_labels(expense_id, label_id) VALUES(?1, ?2)")
                    .bind(id).bind(lid).execute(&state.pool).await?;
            }
        }
    }
    Ok(Redirect::to(f.redirect.as_deref().unwrap_or("/expenses")))
}

pub async fn delete_expense(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Redirect, AppError> {
    sqlx::query("DELETE FROM expenses WHERE id = ?1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(Redirect::to(q.get("redirect").map(|s| s.as_str()).unwrap_or("/expenses")))
}

// ────────── Snapshots ──────────

#[derive(Deserialize)]
pub struct SnapshotForm {
    as_of: String,
    account_id: i64,
    asset_id: i64,
    quantity: f64,
    price_usd: Option<f64>,
    value_usd: f64,
}

pub async fn create_snapshot(
    State(state): State<AppState>,
    Form(f): Form<SnapshotForm>,
) -> Result<Redirect, AppError> {
    sqlx::query(
        "INSERT INTO snapshots(as_of, account_id, asset_id, quantity, price_usd, value_usd, source)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, 'manual')
         ON CONFLICT(as_of, account_id, asset_id) DO UPDATE SET
           quantity  = excluded.quantity,
           price_usd = excluded.price_usd,
           value_usd = excluded.value_usd",
    )
    .bind(&f.as_of)
    .bind(f.account_id)
    .bind(f.asset_id)
    .bind(f.quantity)
    .bind(f.price_usd)
    .bind(f.value_usd)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to("/"))
}

pub async fn delete_snapshot(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError> {
    sqlx::query("DELETE FROM snapshots WHERE id = ?1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(Redirect::to("/"))
}

/// POST /snapshots/trigger — creates snapshots for today from all positions with cached prices.
pub async fn trigger_snapshots(
    State(state): State<AppState>,
) -> Result<Redirect, AppError> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    sqlx::query(
        "INSERT INTO snapshots(as_of, account_id, asset_id, quantity, price_usd, value_usd, source)
         SELECT ?1, p.account_id, p.asset_id, p.quantity, p.last_price,
                p.quantity * COALESCE(p.last_price, 0), 'manual'
         FROM positions p
         WHERE p.quantity > 0
         ON CONFLICT(as_of, account_id, asset_id) DO UPDATE SET
           quantity  = excluded.quantity,
           price_usd = excluded.price_usd,
           value_usd = excluded.value_usd,
           source    = excluded.source",
    )
    .bind(&today)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to("/"))
}

// ────────── Allocation Targets ──────────

#[derive(Deserialize)]
pub struct TargetForm {
    category: String,
    market_mode: Option<String>,
    target_pct: f64,
    notes: Option<String>,
}

pub async fn create_target(
    State(state): State<AppState>,
    Form(f): Form<TargetForm>,
) -> Result<Redirect, AppError> {
    let mode = f.market_mode.as_deref().unwrap_or("crab");
    sqlx::query(
        "INSERT INTO allocation_targets(category, market_mode, target_pct, notes, updated_at)
         VALUES(?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)
         ON CONFLICT(category, market_mode) DO UPDATE SET
           target_pct = excluded.target_pct,
           notes      = excluded.notes,
           updated_at = CURRENT_TIMESTAMP",
    )
    .bind(&f.category)
    .bind(mode)
    .bind(f.target_pct)
    .bind(&f.notes)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to("/targets"))
}

pub async fn delete_target(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError> {
    sqlx::query("DELETE FROM allocation_targets WHERE id = ?1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(Redirect::to("/targets"))
}

// ────────── Edit (update) for income/expenses ──────────

pub async fn update_income(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(f): Form<IncomeForm>,
) -> Result<Redirect, AppError> {
    sqlx::query(
        "UPDATE income SET as_of=?1, salary_usd=?2, bonus_usd=?3, taxes_usd=?4, company=?5
         WHERE id=?6",
    )
    .bind(&f.as_of)
    .bind(f.salary_usd)
    .bind(f.bonus_usd.unwrap_or(0.0))
    .bind(f.taxes_usd.unwrap_or(0.0))
    .bind(&f.company)
    .bind(id)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to("/income"))
}

pub async fn update_expense(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(f): Form<ExpenseForm>,
) -> Result<Redirect, AppError> {
    let category_id = f.category_id.filter(|c| *c > 0);
    sqlx::query(
        "UPDATE expenses SET as_of=?1, amount_usd=?2, place=?3, category_id=?4 WHERE id=?5",
    )
    .bind(&f.as_of)
    .bind(f.amount_usd)
    .bind(&f.place)
    .bind(category_id)
    .bind(id)
    .execute(&state.pool)
    .await?;
    if let Some(raw) = f.label_ids.as_deref() {
        sqlx::query("DELETE FROM expense_labels WHERE expense_id = ?1").bind(id).execute(&state.pool).await?;
        for s in raw.split(',').filter(|s| !s.trim().is_empty()) {
            if let Ok(lid) = s.trim().parse::<i64>() {
                sqlx::query("INSERT OR IGNORE INTO expense_labels(expense_id, label_id) VALUES(?1, ?2)")
                    .bind(id).bind(lid).execute(&state.pool).await?;
            }
        }
    }
    Ok(Redirect::to(f.redirect.as_deref().unwrap_or("/expenses")))
}

// ────────── Positions (form-based upsert) ──────────

#[derive(Deserialize)]
pub struct PositionForm {
    account_id: i64,
    asset_id: i64,
    quantity: f64,
    avg_cost: Option<f64>,
    redirect: Option<String>,
}

pub async fn upsert_position_form(
    State(state): State<AppState>,
    Form(f): Form<PositionForm>,
) -> Result<Redirect, AppError> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    sqlx::query(
        "INSERT INTO positions(account_id, asset_id, quantity, avg_cost, last_price, value_usd, as_of)
         VALUES(?1, ?2, ?3, ?4, 0, 0, ?5)
         ON CONFLICT(account_id, asset_id) DO UPDATE SET
           quantity = excluded.quantity,
           avg_cost = COALESCE(excluded.avg_cost, positions.avg_cost),
           as_of    = excluded.as_of",
    )
    .bind(f.account_id)
    .bind(f.asset_id)
    .bind(f.quantity)
    .bind(f.avg_cost)
    .bind(&today)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to(f.redirect.as_deref().unwrap_or("/positions")))
}

pub async fn delete_position_form(
    State(state): State<AppState>,
    Path((acct, asset)): Path<(i64, i64)>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Redirect, AppError> {
    sqlx::query("DELETE FROM positions WHERE account_id=?1 AND asset_id=?2")
        .bind(acct)
        .bind(asset)
        .execute(&state.pool)
        .await?;
    Ok(Redirect::to(q.get("redirect").map(|s| s.as_str()).unwrap_or("/positions")))
}

// ────────── Snapshot editing ──────────

#[derive(Deserialize)]
pub struct SnapshotEditForm {
    quantity: f64,
    price_usd: Option<f64>,
    value_usd: f64,
    redirect: Option<String>,
}

pub async fn update_snapshot(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(f): Form<SnapshotEditForm>,
) -> Result<Redirect, AppError> {
    sqlx::query(
        "UPDATE snapshots SET quantity=?1, price_usd=?2, value_usd=?3 WHERE id=?4",
    )
    .bind(f.quantity)
    .bind(f.price_usd)
    .bind(f.value_usd)
    .bind(id)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to(f.redirect.as_deref().unwrap_or("/")))
}

// ────────── Categories ──────────

#[derive(Deserialize)]
pub struct CategoryForm {
    name: String,
    parent_id: Option<i64>,
    color: Option<String>,
}

pub async fn create_category(
    State(state): State<AppState>,
    Form(f): Form<CategoryForm>,
) -> Result<Redirect, AppError> {
    // Empty parent_id from a <select> arrives as Some("") — treat as None
    let parent = f.parent_id.filter(|id| *id > 0);
    let color = f.color.filter(|c| !c.is_empty());
    sqlx::query("INSERT INTO categories(name, parent_id, color, active) VALUES(?1, ?2, ?3, 1)")
        .bind(&f.name).bind(parent).bind(&color)
        .execute(&state.pool).await?;
    Ok(Redirect::to("/data?tab=categories"))
}

pub async fn delete_category(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError> {
    let refs: i64 = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM expenses WHERE category_id = ?1)
              + (SELECT COUNT(*) FROM categories WHERE parent_id = ?1)",
    ).bind(id).fetch_one(&state.pool).await?;
    if refs == 0 {
        sqlx::query("DELETE FROM categories WHERE id = ?1").bind(id).execute(&state.pool).await?;
    } else {
        sqlx::query("UPDATE categories SET active = 0 WHERE id = ?1").bind(id).execute(&state.pool).await?;
    }
    Ok(Redirect::to("/data?tab=categories"))
}

// ────────── Labels ──────────

#[derive(Deserialize)]
pub struct LabelForm {
    name: String,
    color: Option<String>,
}

pub async fn create_label(
    State(state): State<AppState>,
    Form(f): Form<LabelForm>,
) -> Result<Redirect, AppError> {
    let color = f.color.filter(|c| !c.is_empty());
    sqlx::query("INSERT INTO labels(name, color, active) VALUES(?1, ?2, 1)")
        .bind(&f.name).bind(&color)
        .execute(&state.pool).await?;
    Ok(Redirect::to("/data?tab=labels"))
}

pub async fn delete_label(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError> {
    let refs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM expense_labels WHERE label_id = ?1")
        .bind(id).fetch_one(&state.pool).await?;
    if refs == 0 {
        sqlx::query("DELETE FROM labels WHERE id = ?1").bind(id).execute(&state.pool).await?;
    } else {
        sqlx::query("UPDATE labels SET active = 0 WHERE id = ?1").bind(id).execute(&state.pool).await?;
    }
    Ok(Redirect::to("/data?tab=labels"))
}
