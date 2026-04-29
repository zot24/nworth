//! Full REST API — JSON CRUD for all entities.
//!
//! All endpoints under /api/v1/
//! GET    /api/v1/{entity}       → list all
//! GET    /api/v1/{entity}/{id}  → get one
//! POST   /api/v1/{entity}       → create
//! PUT    /api/v1/{entity}/{id}  → update
//! DELETE /api/v1/{entity}/{id}  → delete

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{error::AppError, AppState};

/// Result of a smart-delete on accounts/assets. `purged=true` means the row was
/// permanently removed (no references existed); `deactivated=true` means it was
/// soft-deleted (active=0) because referencing snapshots/positions/transactions exist.
#[derive(Serialize)]
pub struct DeleteResult {
    pub id: i64,
    pub purged: bool,
    pub deactivated: bool,
}

// ────────── Accounts ──────────

#[derive(Serialize, sqlx::FromRow)]
pub struct AccountOut {
    pub id: i64,
    pub name: String,
    pub type_code: String,
    pub institution: Option<String>,
    pub chain_code: Option<String>,
    pub active: i64,
    pub notes: Option<String>,
    pub role: String,
}

#[derive(Deserialize)]
pub struct AccountIn {
    pub name: String,
    pub type_code: String,
    pub institution: Option<String>,
    pub chain_code: Option<String>,
    pub notes: Option<String>,
    /// 'investment' | 'operating' | 'property'. Defaults to 'investment'.
    pub role: Option<String>,
}

fn role_or_default(raw: Option<&str>) -> &'static str {
    match raw {
        Some("operating") => "operating",
        Some("property") => "property",
        _ => "investment",
    }
}

pub async fn list_accounts(State(s): State<AppState>) -> Result<Json<Vec<AccountOut>>, AppError> {
    let rows = sqlx::query_as::<_, AccountOut>(
        "SELECT id, name, type_code, institution, chain_code, active, notes, role FROM accounts ORDER BY type_code, name",
    )
    .fetch_all(&s.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn get_account(State(s): State<AppState>, Path(id): Path<i64>) -> Result<Json<AccountOut>, AppError> {
    let row = sqlx::query_as::<_, AccountOut>(
        "SELECT id, name, type_code, institution, chain_code, active, notes, role FROM accounts WHERE id=?1",
    )
    .bind(id)
    .fetch_one(&s.pool)
    .await?;
    Ok(Json(row))
}

pub async fn create_account_json(State(s): State<AppState>, Json(body): Json<AccountIn>) -> Result<(StatusCode, Json<AccountOut>), AppError> {
    let role = role_or_default(body.role.as_deref());
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts(name, type_code, institution, chain_code, active, notes, role) VALUES(?1,?2,?3,?4,1,?5,?6) RETURNING id",
    )
    .bind(&body.name).bind(&body.type_code).bind(&body.institution).bind(&body.chain_code).bind(&body.notes).bind(role)
    .fetch_one(&s.pool).await?;
    let row = sqlx::query_as::<_, AccountOut>("SELECT id, name, type_code, institution, chain_code, active, notes, role FROM accounts WHERE id=?1")
        .bind(id).fetch_one(&s.pool).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn update_account_json(State(s): State<AppState>, Path(id): Path<i64>, Json(body): Json<AccountIn>) -> Result<Json<AccountOut>, AppError> {
    let role = role_or_default(body.role.as_deref());
    sqlx::query("UPDATE accounts SET name=?1, type_code=?2, institution=?3, chain_code=?4, notes=?5, role=?6 WHERE id=?7")
        .bind(&body.name).bind(&body.type_code).bind(&body.institution).bind(&body.chain_code).bind(&body.notes).bind(role).bind(id)
        .execute(&s.pool).await?;
    let row = sqlx::query_as::<_, AccountOut>("SELECT id, name, type_code, institution, chain_code, active, notes, role FROM accounts WHERE id=?1")
        .bind(id).fetch_one(&s.pool).await?;
    Ok(Json(row))
}

pub async fn delete_account_json(State(s): State<AppState>, Path(id): Path<i64>) -> Result<Json<DeleteResult>, AppError> {
    let purged = crate::routes::crud::smart_delete_account(&s.pool, id).await?;
    Ok(Json(DeleteResult { id, purged, deactivated: !purged }))
}

// ────────── Assets ──────────

#[derive(Serialize, sqlx::FromRow)]
pub struct AssetOut {
    pub id: i64,
    pub symbol: String,
    pub name: Option<String>,
    pub type_code: String,
    pub chain_code: Option<String>,
    pub risk_code: Option<String>,
    pub coingecko_id: Option<String>,
    pub yahoo_ticker: Option<String>,
    pub is_stable: i64,
    pub active: i64,
}

#[derive(Deserialize)]
pub struct AssetIn {
    pub symbol: String,
    pub name: Option<String>,
    pub type_code: String,
    pub chain_code: Option<String>,
    pub risk_code: Option<String>,
    pub coingecko_id: Option<String>,
    pub yahoo_ticker: Option<String>,
    pub is_stable: Option<bool>,
}

pub async fn list_assets(State(s): State<AppState>) -> Result<Json<Vec<AssetOut>>, AppError> {
    let rows = sqlx::query_as::<_, AssetOut>(
        "SELECT id, symbol, name, type_code, chain_code, risk_code, coingecko_id, yahoo_ticker, is_stable, active FROM assets ORDER BY type_code, symbol",
    ).fetch_all(&s.pool).await?;
    Ok(Json(rows))
}

pub async fn get_asset(State(s): State<AppState>, Path(id): Path<i64>) -> Result<Json<AssetOut>, AppError> {
    let row = sqlx::query_as::<_, AssetOut>(
        "SELECT id, symbol, name, type_code, chain_code, risk_code, coingecko_id, yahoo_ticker, is_stable, active FROM assets WHERE id=?1",
    ).bind(id).fetch_one(&s.pool).await?;
    Ok(Json(row))
}

pub async fn create_asset_json(State(s): State<AppState>, Json(body): Json<AssetIn>) -> Result<(StatusCode, Json<AssetOut>), AppError> {
    let is_stable: i64 = if body.is_stable.unwrap_or(false) { 1 } else { 0 };
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO assets(symbol, name, type_code, chain_code, risk_code, coingecko_id, yahoo_ticker, is_stable, active) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,1) RETURNING id",
    ).bind(&body.symbol).bind(&body.name).bind(&body.type_code).bind(&body.chain_code).bind(&body.risk_code)
      .bind(&body.coingecko_id).bind(&body.yahoo_ticker).bind(is_stable)
      .fetch_one(&s.pool).await?;
    let row = sqlx::query_as::<_, AssetOut>("SELECT id, symbol, name, type_code, chain_code, risk_code, coingecko_id, yahoo_ticker, is_stable, active FROM assets WHERE id=?1")
        .bind(id).fetch_one(&s.pool).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn update_asset_json(State(s): State<AppState>, Path(id): Path<i64>, Json(body): Json<AssetIn>) -> Result<Json<AssetOut>, AppError> {
    let is_stable: i64 = if body.is_stable.unwrap_or(false) { 1 } else { 0 };
    sqlx::query("UPDATE assets SET symbol=?1, name=?2, type_code=?3, chain_code=?4, risk_code=?5, coingecko_id=?6, yahoo_ticker=?7, is_stable=?8 WHERE id=?9")
        .bind(&body.symbol).bind(&body.name).bind(&body.type_code).bind(&body.chain_code).bind(&body.risk_code)
        .bind(&body.coingecko_id).bind(&body.yahoo_ticker).bind(is_stable).bind(id)
        .execute(&s.pool).await?;
    let row = sqlx::query_as::<_, AssetOut>("SELECT id, symbol, name, type_code, chain_code, risk_code, coingecko_id, yahoo_ticker, is_stable, active FROM assets WHERE id=?1")
        .bind(id).fetch_one(&s.pool).await?;
    Ok(Json(row))
}

pub async fn delete_asset_json(State(s): State<AppState>, Path(id): Path<i64>) -> Result<Json<DeleteResult>, AppError> {
    let purged = crate::routes::crud::smart_delete_asset(&s.pool, id).await?;
    Ok(Json(DeleteResult { id, purged, deactivated: !purged }))
}

// ────────── Snapshots ──────────

#[derive(Serialize, sqlx::FromRow)]
pub struct SnapshotOut {
    pub id: i64,
    pub as_of: String,
    pub account_id: i64,
    pub asset_id: i64,
    pub quantity: f64,
    pub price_usd: Option<f64>,
    pub value_usd: f64,
    pub source: Option<String>,
}

#[derive(Deserialize)]
pub struct SnapshotIn {
    pub as_of: String,
    pub account_id: i64,
    pub asset_id: i64,
    pub quantity: f64,
    pub price_usd: Option<f64>,
    pub value_usd: f64,
}

pub async fn list_snapshots(State(s): State<AppState>, axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>) -> Result<Json<Vec<SnapshotOut>>, AppError> {
    let as_of = q.get("as_of");
    let rows = if let Some(date) = as_of {
        sqlx::query_as::<_, SnapshotOut>(
            "SELECT id, as_of, account_id, asset_id, quantity * 1.0 as quantity, price_usd, value_usd * 1.0 as value_usd, source FROM snapshots WHERE as_of=?1 ORDER BY value_usd DESC",
        ).bind(date).fetch_all(&s.pool).await?
    } else {
        // Return latest date only by default
        sqlx::query_as::<_, SnapshotOut>(
            "SELECT id, as_of, account_id, asset_id, quantity * 1.0 as quantity, price_usd, value_usd * 1.0 as value_usd, source FROM snapshots WHERE as_of=(SELECT MAX(as_of) FROM snapshots) ORDER BY value_usd DESC",
        ).fetch_all(&s.pool).await?
    };
    Ok(Json(rows))
}

pub async fn create_snapshot_json(State(s): State<AppState>, Json(body): Json<SnapshotIn>) -> Result<(StatusCode, Json<SnapshotOut>), AppError> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO snapshots(as_of, account_id, asset_id, quantity, price_usd, value_usd, source) VALUES(?1,?2,?3,?4,?5,?6,'api')
         ON CONFLICT(as_of, account_id, asset_id) DO UPDATE SET quantity=excluded.quantity, price_usd=excluded.price_usd, value_usd=excluded.value_usd RETURNING id",
    ).bind(&body.as_of).bind(body.account_id).bind(body.asset_id).bind(body.quantity).bind(body.price_usd).bind(body.value_usd)
     .fetch_one(&s.pool).await?;
    let row = sqlx::query_as::<_, SnapshotOut>("SELECT id, as_of, account_id, asset_id, quantity * 1.0 as quantity, price_usd, value_usd * 1.0 as value_usd, source FROM snapshots WHERE id=?1")
        .bind(id).fetch_one(&s.pool).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn delete_snapshot_json(State(s): State<AppState>, Path(id): Path<i64>) -> Result<StatusCode, AppError> {
    sqlx::query("DELETE FROM snapshots WHERE id=?1").bind(id).execute(&s.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ────────── Positions ──────────

#[derive(Serialize, sqlx::FromRow)]
pub struct PositionOut {
    pub account_id: i64,
    pub asset_id: i64,
    pub quantity: f64,
    pub avg_cost: Option<f64>,
    pub last_price: Option<f64>,
    pub value_usd: f64,
    pub as_of: String,
}

#[derive(Deserialize)]
pub struct PositionIn {
    pub account_id: i64,
    pub asset_id: i64,
    pub quantity: f64,
    pub avg_cost: Option<f64>,
}

pub async fn list_positions(State(s): State<AppState>) -> Result<Json<Vec<PositionOut>>, AppError> {
    // last_price + value_usd are derived live by joining assets.
    let rows = sqlx::query_as::<_, PositionOut>(
        "SELECT p.account_id, p.asset_id, p.quantity * 1.0 as quantity, p.avg_cost,
                a.last_price as last_price,
                p.quantity * COALESCE(a.last_price, 0) as value_usd,
                p.as_of
         FROM positions p
         JOIN assets a ON a.id = p.asset_id
         ORDER BY value_usd DESC",
    ).fetch_all(&s.pool).await?;
    Ok(Json(rows))
}

pub async fn upsert_position(State(s): State<AppState>, Json(body): Json<PositionIn>) -> Result<(StatusCode, Json<PositionOut>), AppError> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    sqlx::query(
        "INSERT INTO positions(account_id, asset_id, quantity, avg_cost, as_of) VALUES(?1,?2,?3,?4,?5)
         ON CONFLICT(account_id, asset_id) DO UPDATE SET quantity=excluded.quantity, avg_cost=excluded.avg_cost, as_of=excluded.as_of",
    ).bind(body.account_id).bind(body.asset_id).bind(body.quantity).bind(body.avg_cost).bind(&today)
     .execute(&s.pool).await?;
    let row = sqlx::query_as::<_, PositionOut>(
        "SELECT p.account_id, p.asset_id, p.quantity * 1.0 as quantity, p.avg_cost,
                a.last_price as last_price,
                p.quantity * COALESCE(a.last_price, 0) as value_usd,
                p.as_of
         FROM positions p
         JOIN assets a ON a.id = p.asset_id
         WHERE p.account_id=?1 AND p.asset_id=?2",
    ).bind(body.account_id).bind(body.asset_id).fetch_one(&s.pool).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn delete_position(State(s): State<AppState>, Path((acct, asset)): Path<(i64, i64)>) -> Result<StatusCode, AppError> {
    sqlx::query("DELETE FROM positions WHERE account_id=?1 AND asset_id=?2").bind(acct).bind(asset).execute(&s.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ────────── Income ──────────

#[derive(Serialize, sqlx::FromRow)]
pub struct IncomeOut {
    pub id: i64,
    pub as_of: String,
    pub salary_usd: f64,
    pub per_year_usd: f64,
    pub bonus_usd: f64,
    pub taxes_usd: f64,
    pub company: Option<String>,
}

#[derive(Deserialize)]
pub struct IncomeIn {
    pub as_of: String,
    pub salary_usd: f64,
    pub bonus_usd: Option<f64>,
    pub taxes_usd: Option<f64>,
    pub company: Option<String>,
}

pub async fn list_income(State(s): State<AppState>) -> Result<Json<Vec<IncomeOut>>, AppError> {
    let rows = sqlx::query_as::<_, IncomeOut>("SELECT id, as_of, salary_usd, per_year_usd, bonus_usd, taxes_usd, company FROM income ORDER BY as_of DESC")
        .fetch_all(&s.pool).await?;
    Ok(Json(rows))
}

pub async fn create_income_json(State(s): State<AppState>, Json(body): Json<IncomeIn>) -> Result<(StatusCode, Json<IncomeOut>), AppError> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO income(as_of, salary_usd, per_year_usd, bonus_usd, taxes_usd, company, source) VALUES(?1,?2,0,?3,?4,?5,'api')
         ON CONFLICT(as_of) DO UPDATE SET salary_usd=excluded.salary_usd, bonus_usd=excluded.bonus_usd, taxes_usd=excluded.taxes_usd, company=excluded.company RETURNING id",
    ).bind(&body.as_of).bind(body.salary_usd).bind(body.bonus_usd.unwrap_or(0.0)).bind(body.taxes_usd.unwrap_or(0.0)).bind(&body.company)
     .fetch_one(&s.pool).await?;
    let row = sqlx::query_as::<_, IncomeOut>("SELECT id, as_of, salary_usd, per_year_usd, bonus_usd, taxes_usd, company FROM income WHERE id=?1")
        .bind(id).fetch_one(&s.pool).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn update_income_json(State(s): State<AppState>, Path(id): Path<i64>, Json(body): Json<IncomeIn>) -> Result<Json<IncomeOut>, AppError> {
    sqlx::query("UPDATE income SET as_of=?1, salary_usd=?2, bonus_usd=?3, taxes_usd=?4, company=?5 WHERE id=?6")
        .bind(&body.as_of).bind(body.salary_usd).bind(body.bonus_usd.unwrap_or(0.0)).bind(body.taxes_usd.unwrap_or(0.0)).bind(&body.company).bind(id)
        .execute(&s.pool).await?;
    let row = sqlx::query_as::<_, IncomeOut>("SELECT id, as_of, salary_usd, per_year_usd, bonus_usd, taxes_usd, company FROM income WHERE id=?1")
        .bind(id).fetch_one(&s.pool).await?;
    Ok(Json(row))
}

pub async fn delete_income_json(State(s): State<AppState>, Path(id): Path<i64>) -> Result<StatusCode, AppError> {
    sqlx::query("DELETE FROM income WHERE id=?1").bind(id).execute(&s.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ────────── Expenses ──────────

#[derive(Serialize)]
pub struct ExpenseOut {
    pub id: i64,
    pub as_of: String,
    pub amount_usd: f64,
    pub place: Option<String>,
    pub notes: Option<String>,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub labels: Vec<LabelRef>,
}

#[derive(Serialize)]
pub struct LabelRef {
    pub id: i64,
    pub name: String,
}

#[derive(Deserialize)]
pub struct ExpenseIn {
    pub as_of: String,
    pub amount_usd: f64,
    pub place: Option<String>,
    pub category_id: Option<i64>,
    pub label_ids: Option<Vec<i64>>,
}

async fn load_expense(pool: &sqlx::SqlitePool, id: i64) -> Result<ExpenseOut, sqlx::Error> {
    let row: (i64, String, f64, Option<String>, Option<String>, Option<i64>, Option<String>) = sqlx::query_as(
        "SELECT e.id, e.as_of, e.amount_usd * 1.0, e.place, e.notes, e.category_id, c.name
         FROM expenses e LEFT JOIN categories c ON c.id = e.category_id
         WHERE e.id = ?1",
    ).bind(id).fetch_one(pool).await?;
    let labels: Vec<LabelRef> = sqlx::query_as::<_, (i64, String)>(
        "SELECT l.id, l.name FROM labels l
         JOIN expense_labels el ON el.label_id = l.id
         WHERE el.expense_id = ?1 ORDER BY l.name",
    ).bind(id).fetch_all(pool).await?
        .into_iter().map(|(id, name)| LabelRef { id, name }).collect();
    Ok(ExpenseOut {
        id: row.0, as_of: row.1, amount_usd: row.2, place: row.3, notes: row.4,
        category_id: row.5, category_name: row.6, labels,
    })
}

async fn replace_expense_labels(pool: &sqlx::SqlitePool, expense_id: i64, label_ids: &[i64]) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM expense_labels WHERE expense_id = ?1").bind(expense_id).execute(pool).await?;
    for label_id in label_ids {
        sqlx::query("INSERT OR IGNORE INTO expense_labels(expense_id, label_id) VALUES(?1, ?2)")
            .bind(expense_id).bind(label_id).execute(pool).await?;
    }
    Ok(())
}

pub async fn list_expenses(State(s): State<AppState>) -> Result<Json<Vec<ExpenseOut>>, AppError> {
    let ids: Vec<(i64,)> = sqlx::query_as("SELECT id FROM expenses ORDER BY as_of DESC")
        .fetch_all(&s.pool).await?;
    let mut out = Vec::with_capacity(ids.len());
    for (id,) in ids { out.push(load_expense(&s.pool, id).await?); }
    Ok(Json(out))
}

pub async fn create_expense_json(State(s): State<AppState>, Json(body): Json<ExpenseIn>) -> Result<(StatusCode, Json<ExpenseOut>), AppError> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO expenses(as_of, amount_usd, place, source, category_id) VALUES(?1,?2,?3,'api',?4)
         ON CONFLICT(as_of) DO UPDATE SET amount_usd=excluded.amount_usd, place=excluded.place, category_id=excluded.category_id RETURNING id",
    ).bind(&body.as_of).bind(body.amount_usd).bind(&body.place).bind(body.category_id)
     .fetch_one(&s.pool).await?;
    if let Some(label_ids) = &body.label_ids {
        replace_expense_labels(&s.pool, id, label_ids).await?;
    }
    Ok((StatusCode::CREATED, Json(load_expense(&s.pool, id).await?)))
}

pub async fn update_expense_json(State(s): State<AppState>, Path(id): Path<i64>, Json(body): Json<ExpenseIn>) -> Result<Json<ExpenseOut>, AppError> {
    sqlx::query("UPDATE expenses SET as_of=?1, amount_usd=?2, place=?3, category_id=?4 WHERE id=?5")
        .bind(&body.as_of).bind(body.amount_usd).bind(&body.place).bind(body.category_id).bind(id)
        .execute(&s.pool).await?;
    if let Some(label_ids) = &body.label_ids {
        replace_expense_labels(&s.pool, id, label_ids).await?;
    }
    Ok(Json(load_expense(&s.pool, id).await?))
}

pub async fn delete_expense_json(State(s): State<AppState>, Path(id): Path<i64>) -> Result<StatusCode, AppError> {
    sqlx::query("DELETE FROM expenses WHERE id=?1").bind(id).execute(&s.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ────────── Categories ──────────

#[derive(Serialize, sqlx::FromRow)]
pub struct CategoryOut {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub color: Option<String>,
    pub active: i64,
}

#[derive(Deserialize)]
pub struct CategoryIn {
    pub name: String,
    pub parent_id: Option<i64>,
    pub color: Option<String>,
}

pub async fn list_categories(State(s): State<AppState>) -> Result<Json<Vec<CategoryOut>>, AppError> {
    let rows = sqlx::query_as::<_, CategoryOut>(
        "SELECT id, name, parent_id, color, active FROM categories ORDER BY COALESCE(parent_id, id), name",
    ).fetch_all(&s.pool).await?;
    Ok(Json(rows))
}

pub async fn create_category_json(State(s): State<AppState>, Json(body): Json<CategoryIn>) -> Result<(StatusCode, Json<CategoryOut>), AppError> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO categories(name, parent_id, color, active) VALUES(?1,?2,?3,1) RETURNING id",
    ).bind(&body.name).bind(body.parent_id).bind(&body.color)
     .fetch_one(&s.pool).await?;
    let row = sqlx::query_as::<_, CategoryOut>("SELECT id, name, parent_id, color, active FROM categories WHERE id=?1")
        .bind(id).fetch_one(&s.pool).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn update_category_json(State(s): State<AppState>, Path(id): Path<i64>, Json(body): Json<CategoryIn>) -> Result<Json<CategoryOut>, AppError> {
    sqlx::query("UPDATE categories SET name=?1, parent_id=?2, color=?3 WHERE id=?4")
        .bind(&body.name).bind(body.parent_id).bind(&body.color).bind(id)
        .execute(&s.pool).await?;
    let row = sqlx::query_as::<_, CategoryOut>("SELECT id, name, parent_id, color, active FROM categories WHERE id=?1")
        .bind(id).fetch_one(&s.pool).await?;
    Ok(Json(row))
}

/// Smart delete — hard-delete when no expenses or child categories reference it; else mark inactive.
pub async fn delete_category_json(State(s): State<AppState>, Path(id): Path<i64>) -> Result<Json<DeleteResult>, AppError> {
    let refs: i64 = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM expenses WHERE category_id = ?1)
              + (SELECT COUNT(*) FROM categories WHERE parent_id = ?1)",
    ).bind(id).fetch_one(&s.pool).await?;
    let purged = if refs == 0 {
        sqlx::query("DELETE FROM categories WHERE id=?1").bind(id).execute(&s.pool).await?;
        true
    } else {
        sqlx::query("UPDATE categories SET active=0 WHERE id=?1").bind(id).execute(&s.pool).await?;
        false
    };
    Ok(Json(DeleteResult { id, purged, deactivated: !purged }))
}

// ────────── Labels ──────────

#[derive(Serialize, sqlx::FromRow)]
pub struct LabelOut {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
    pub active: i64,
}

#[derive(Deserialize)]
pub struct LabelIn {
    pub name: String,
    pub color: Option<String>,
}

pub async fn list_labels(State(s): State<AppState>) -> Result<Json<Vec<LabelOut>>, AppError> {
    let rows = sqlx::query_as::<_, LabelOut>("SELECT id, name, color, active FROM labels ORDER BY name")
        .fetch_all(&s.pool).await?;
    Ok(Json(rows))
}

pub async fn create_label_json(State(s): State<AppState>, Json(body): Json<LabelIn>) -> Result<(StatusCode, Json<LabelOut>), AppError> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO labels(name, color, active) VALUES(?1,?2,1) RETURNING id",
    ).bind(&body.name).bind(&body.color).fetch_one(&s.pool).await?;
    let row = sqlx::query_as::<_, LabelOut>("SELECT id, name, color, active FROM labels WHERE id=?1")
        .bind(id).fetch_one(&s.pool).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn update_label_json(State(s): State<AppState>, Path(id): Path<i64>, Json(body): Json<LabelIn>) -> Result<Json<LabelOut>, AppError> {
    sqlx::query("UPDATE labels SET name=?1, color=?2 WHERE id=?3")
        .bind(&body.name).bind(&body.color).bind(id).execute(&s.pool).await?;
    let row = sqlx::query_as::<_, LabelOut>("SELECT id, name, color, active FROM labels WHERE id=?1")
        .bind(id).fetch_one(&s.pool).await?;
    Ok(Json(row))
}

pub async fn delete_label_json(State(s): State<AppState>, Path(id): Path<i64>) -> Result<Json<DeleteResult>, AppError> {
    let refs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM expense_labels WHERE label_id = ?1")
        .bind(id).fetch_one(&s.pool).await?;
    let purged = if refs == 0 {
        sqlx::query("DELETE FROM labels WHERE id=?1").bind(id).execute(&s.pool).await?;
        true
    } else {
        sqlx::query("UPDATE labels SET active=0 WHERE id=?1").bind(id).execute(&s.pool).await?;
        false
    };
    Ok(Json(DeleteResult { id, purged, deactivated: !purged }))
}

// ────────── Allocation Targets ──────────

#[derive(Serialize, sqlx::FromRow)]
pub struct TargetOut {
    pub id: i64,
    pub category: String,
    pub market_mode: String,
    pub target_pct: f64,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct TargetIn {
    pub category: String,
    pub market_mode: Option<String>,
    pub target_pct: f64,
    pub notes: Option<String>,
}

pub async fn list_targets(State(s): State<AppState>) -> Result<Json<Vec<TargetOut>>, AppError> {
    let rows = sqlx::query_as::<_, TargetOut>("SELECT id, category, market_mode, target_pct, notes FROM allocation_targets ORDER BY category, market_mode")
        .fetch_all(&s.pool).await?;
    Ok(Json(rows))
}

pub async fn create_target_json(State(s): State<AppState>, Json(body): Json<TargetIn>) -> Result<(StatusCode, Json<TargetOut>), AppError> {
    let mode = body.market_mode.as_deref().unwrap_or("crab");
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO allocation_targets(category, market_mode, target_pct, notes, updated_at) VALUES(?1,?2,?3,?4,CURRENT_TIMESTAMP)
         ON CONFLICT(category, market_mode) DO UPDATE SET target_pct=excluded.target_pct, notes=excluded.notes, updated_at=CURRENT_TIMESTAMP RETURNING id",
    ).bind(&body.category).bind(mode).bind(body.target_pct).bind(&body.notes)
     .fetch_one(&s.pool).await?;
    let row = sqlx::query_as::<_, TargetOut>("SELECT id, category, market_mode, target_pct, notes FROM allocation_targets WHERE id=?1")
        .bind(id).fetch_one(&s.pool).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn delete_target_json(State(s): State<AppState>, Path(id): Path<i64>) -> Result<StatusCode, AppError> {
    sqlx::query("DELETE FROM allocation_targets WHERE id=?1").bind(id).execute(&s.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ────────── Snapshot Trigger ──────────

#[derive(Serialize)]
pub struct TriggerResult {
    pub date: String,
    pub snapshots_created: u64,
}

pub async fn trigger_snapshot_json(State(s): State<AppState>) -> Result<Json<TriggerResult>, AppError> {
    // Anchored to first-of-month so this matches the pricefeed's monthly cadence.
    let anchor = chrono::Utc::now().format("%Y-%m-01").to_string();
    let result = sqlx::query(
        "INSERT INTO snapshots(as_of, account_id, asset_id, quantity, price_usd, value_usd, source)
         SELECT ?1, p.account_id, p.asset_id, p.quantity, a.last_price, p.quantity * COALESCE(a.last_price, 0), 'api'
         FROM positions p JOIN assets a ON a.id = p.asset_id
         WHERE p.quantity > 0
         ON CONFLICT(as_of, account_id, asset_id) DO UPDATE SET quantity=excluded.quantity, price_usd=excluded.price_usd, value_usd=excluded.value_usd",
    ).bind(&anchor).execute(&s.pool).await?;
    Ok(Json(TriggerResult { date: anchor, snapshots_created: result.rows_affected() }))
}

// ────────── Settings (key/value) ──────────

#[derive(Serialize, sqlx::FromRow)]
pub struct SettingOut {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
pub struct SettingIn {
    pub value: String,
}

pub async fn list_settings(State(s): State<AppState>) -> Result<Json<Vec<SettingOut>>, AppError> {
    let rows = sqlx::query_as::<_, SettingOut>(
        "SELECT key, value, updated_at FROM settings ORDER BY key",
    ).fetch_all(&s.pool).await?;
    Ok(Json(rows))
}

pub async fn update_setting_json(
    State(s): State<AppState>,
    Path(key): Path<String>,
    Json(body): Json<SettingIn>,
) -> Result<Json<SettingOut>, AppError> {
    crate::routes::settings::set_setting(&s.pool, &key, &body.value).await?;
    let row = sqlx::query_as::<_, SettingOut>(
        "SELECT key, value, updated_at FROM settings WHERE key = ?1",
    ).bind(&key).fetch_one(&s.pool).await?;
    Ok(Json(row))
}
