use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Query, State};
use serde::Deserialize;

use crate::{error::AppError, AppState};

#[derive(Debug)]
pub struct AssetRow {
    pub id: i64,
    pub symbol: String,
    pub name: String,
    pub type_code: String,
    pub chain_code: String,
    pub risk_code: String,
    pub coingecko_id: String,
    pub yahoo_ticker: String,
    pub active: i64,
}

#[derive(Debug)]
pub struct SnapshotRow {
    pub id: i64,
    pub as_of: String,
    pub account_name: String,
    pub asset_symbol: String,
    pub quantity: f64,
    pub price_usd: f64,
    pub value_usd: f64,
    pub source: String,
}

#[derive(Debug)]
pub struct PositionRow {
    pub account_name: String,
    pub asset_symbol: String,
    pub account_id: i64,
    pub asset_id: i64,
    pub quantity: f64,
    pub avg_cost: f64,
    pub last_price: f64,
    pub value_usd: f64,
    pub apy_pct: f64,
}

#[derive(Debug)]
pub struct AccountRow {
    pub id: i64,
    pub name: String,
    pub type_code: String,
    pub institution: String,
    pub chain_code: String,
    pub active: i64,
    pub notes: String,
    pub role: String,
}

#[derive(Debug)]
pub struct IncomeRow {
    pub id: i64,
    pub as_of: String,
    pub salary_usd: f64,
    pub bonus_usd: f64,
    pub taxes_usd: f64,
    pub company: String,
}

#[derive(Debug)]
pub struct ExpenseRow {
    pub id: i64,
    pub as_of: String,
    pub amount_usd: f64,
    pub place: String,
    pub notes: String,
    pub category_id: Option<i64>,
    /// Mirror of `category_id` flattened to 0 when None, so the template's
    /// `c.id == e.selected_category_id` comparison works without askama
    /// needing to deref an Option.
    pub selected_category_id: i64,
    pub category_name: String,
    pub label_ids_csv: String,
    pub labels_display: String,
}

#[derive(Debug, Clone)]
pub struct CategoryRow {
    pub id: i64,
    pub name: String,
    pub display_name: String, // "Food / Restaurants" — pre-rendered with parent path
    pub parent_id: Option<i64>,
    pub color: String,
    pub active: i64,
}

#[derive(Debug)]
pub struct LabelRow {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub active: i64,
    pub use_count: i64,
}

#[derive(Debug)]
pub struct TargetGroupRow {
    pub category: String,
    pub bull_pct: f64,
    pub crab_pct: f64,
    pub bear_pct: f64,
}

#[derive(Deserialize)]
pub struct DataQuery {
    pub tab: Option<String>,
    pub asset_type: Option<String>,
    pub as_of: Option<String>,
    /// Year filter for income + expenses tabs ("YYYY" or "all").
    pub year: Option<String>,
}

/// One pickable year + whether it's the active selection. Pre-built in the
/// handler so the template doesn't have to compare String to &String (askama
/// doesn't support String == &String / `*` deref in expressions).
pub struct YearOption {
    pub year: String,
    pub selected: bool,
}

#[derive(Template)]
#[template(path = "data.html")]
struct DataTemplate {
    is_assets: bool,
    is_snapshots: bool,
    is_positions: bool,
    is_accounts: bool,
    is_income: bool,
    is_expenses: bool,
    is_targets: bool,
    is_categories: bool,
    is_labels: bool,
    assets: Vec<AssetRow>,
    snapshots: Vec<SnapshotRow>,
    positions: Vec<PositionRow>,
    accounts: Vec<AccountRow>,
    income_rows: Vec<IncomeRow>,
    expense_rows: Vec<ExpenseRow>,
    target_groups: Vec<TargetGroupRow>,
    target_total_bull: f64,
    target_total_crab: f64,
    target_total_bear: f64,
    categories: Vec<CategoryRow>,
    labels: Vec<LabelRow>,
    snapshot_dates: Vec<String>,
    selected_date: String,
    year_all_selected: bool,
    income_year_options: Vec<YearOption>,
    expense_year_options: Vec<YearOption>,
}

pub async fn index(
    State(state): State<AppState>,
    Query(q): Query<DataQuery>,
) -> Result<impl IntoResponse, AppError> {
    let tab = q.tab.unwrap_or_else(|| "assets".to_string());
    let selected_asset_type = q.asset_type.unwrap_or_default();

    // Year filter (income + expenses tabs). "all" or absent → no filter.
    // Bound by valid YYYY pattern to keep the SQL clean.
    let year_filter: Option<String> = q.year.as_deref()
        .filter(|y| *y != "all" && y.len() == 4 && y.chars().all(|c| c.is_ascii_digit()))
        .map(|y| y.to_string());
    let (year_lo, year_hi) = match &year_filter {
        Some(y) => (format!("{y}-01-01"), format!("{y}-12-31")),
        None => ("0000-01-01".to_string(), "9999-12-31".to_string()),
    };
    let year_all_selected = year_filter.is_none();

    // Distinct years for each pickable tab — used to render the year picker.
    let income_years: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT SUBSTR(as_of, 1, 4) AS y FROM income ORDER BY y DESC",
    ).fetch_all(&state.pool).await?;
    let expense_years: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT SUBSTR(as_of, 1, 4) AS y FROM expenses ORDER BY y DESC",
    ).fetch_all(&state.pool).await?;
    let to_options = |years: Vec<String>| -> Vec<YearOption> {
        years.into_iter()
            .map(|y| YearOption {
                selected: year_filter.as_deref() == Some(y.as_str()),
                year: y,
            })
            .collect()
    };
    let income_year_options = to_options(income_years);
    let expense_year_options = to_options(expense_years);

    // Assets — all types including 'owned'. The /data editor is the one place
    // where assets are created and edited; owned things (Car, Apartment, …)
    // live alongside crypto/stock/fiat rows here. The type dropdown below
    // therefore needs to include 'owned' so editing doesn't silently flip type.
    let asset_rows: Vec<(i64, String, Option<String>, String, Option<String>, Option<String>, Option<String>, Option<String>, i64)> = sqlx::query_as(
        "SELECT id, symbol, name, type_code, chain_code, risk_code, coingecko_id, yahoo_ticker, active
         FROM assets ORDER BY active DESC, type_code, symbol",
    )
    .fetch_all(&state.pool)
    .await?;

    let assets: Vec<AssetRow> = asset_rows
        .into_iter()
        .filter(|r| selected_asset_type.is_empty() || r.3 == selected_asset_type)
        .map(|(id, symbol, name, type_code, chain_code, risk_code, coingecko_id, yahoo_ticker, active)| AssetRow {
            id, symbol,
            name: name.unwrap_or_default(),
            type_code,
            chain_code: chain_code.unwrap_or_default(),
            risk_code: risk_code.unwrap_or_default(),
            coingecko_id: coingecko_id.unwrap_or_default(),
            yahoo_ticker: yahoo_ticker.unwrap_or_default(),
            active,
        })
        .collect();

    // Snapshot dates for filter
    let snapshot_dates: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT as_of FROM snapshots ORDER BY as_of DESC LIMIT 24",
    )
    .fetch_all(&state.pool)
    .await?;

    let selected_date = q.as_of.unwrap_or_else(|| snapshot_dates.first().cloned().unwrap_or_default());

    // Snapshots for selected date
    let snap_rows: Vec<(i64, String, String, String, f64, Option<f64>, f64, Option<String>)> = sqlx::query_as(
        "SELECT s.id, s.as_of, ac.name, a.symbol, s.quantity, s.price_usd, s.value_usd, s.source
         FROM snapshots s
         JOIN accounts ac ON ac.id = s.account_id
         JOIN assets a ON a.id = s.asset_id
         WHERE s.as_of = ?1
         ORDER BY s.value_usd DESC",
    )
    .bind(&selected_date)
    .fetch_all(&state.pool)
    .await?;

    let snapshots: Vec<SnapshotRow> = snap_rows
        .into_iter()
        .map(|(id, as_of, account_name, asset_symbol, quantity, price_usd, value_usd, source)| SnapshotRow {
            id, as_of, account_name, asset_symbol, quantity,
            price_usd: price_usd.unwrap_or(0.0),
            value_usd,
            source: source.unwrap_or_default(),
        })
        .collect();

    // Positions
    let pos_rows: Vec<(String, String, i64, i64, f64, f64, f64, f64, f64)> = sqlx::query_as(
        "SELECT ac.name, a.symbol, p.account_id, p.asset_id,
                p.quantity * 1.0, COALESCE(p.avg_cost * 1.0, 0.0),
                COALESCE(a.last_price * 1.0, 0.0),
                p.quantity * COALESCE(a.last_price, 0.0) AS value_usd,
                p.apy_pct * 1.0
         FROM positions p
         JOIN accounts ac ON ac.id = p.account_id
         JOIN assets a ON a.id = p.asset_id
         ORDER BY value_usd DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    let positions: Vec<PositionRow> = pos_rows
        .into_iter()
        .map(|(account_name, asset_symbol, account_id, asset_id, quantity, avg_cost, last_price, value_usd, apy_pct)| PositionRow {
            account_name, asset_symbol, account_id, asset_id, quantity, avg_cost, last_price, value_usd, apy_pct,
        })
        .collect();

    // Accounts — all
    let acct_rows: Vec<(i64, String, String, Option<String>, Option<String>, i64, Option<String>, String)> = sqlx::query_as(
        "SELECT id, name, type_code, institution, chain_code, active, notes, role
         FROM accounts ORDER BY active DESC, type_code, name",
    )
    .fetch_all(&state.pool)
    .await?;

    let accounts: Vec<AccountRow> = acct_rows
        .into_iter()
        .map(|(id, name, type_code, institution, chain_code, active, notes, role)| AccountRow {
            id, name, type_code,
            institution: institution.unwrap_or_default(),
            chain_code: chain_code.unwrap_or_default(),
            active,
            notes: notes.unwrap_or_default(),
            role,
        })
        .collect();

    // Income — filtered by year when set; the LIMIT 60 cap stays only when "all".
    let inc_rows: Vec<(i64, String, f64, f64, f64, Option<String>)> = sqlx::query_as(
        "SELECT id, as_of, salary_usd * 1.0, bonus_usd * 1.0, taxes_usd * 1.0, company
         FROM income
         WHERE as_of >= ?1 AND as_of <= ?2
         ORDER BY as_of DESC LIMIT 240",
    ).bind(&year_lo).bind(&year_hi).fetch_all(&state.pool).await?;
    let income_rows: Vec<IncomeRow> = inc_rows.into_iter()
        .map(|(id, as_of, salary_usd, bonus_usd, taxes_usd, company)| IncomeRow {
            id, as_of, salary_usd, bonus_usd, taxes_usd,
            company: company.unwrap_or_default(),
        })
        .collect();

    // Categories — load once for both the categories tab and the expense tab dropdown
    let cat_raw: Vec<(i64, String, Option<i64>, Option<String>, i64)> = sqlx::query_as(
        "SELECT id, name, parent_id, color, active FROM categories
         ORDER BY active DESC, COALESCE(parent_id, id), name",
    ).fetch_all(&state.pool).await?;
    let by_id: std::collections::HashMap<i64, String> = cat_raw.iter().map(|(id, n, _, _, _)| (*id, n.clone())).collect();
    let categories: Vec<CategoryRow> = cat_raw.into_iter().map(|(id, name, parent_id, color, active)| {
        let display_name = match parent_id.and_then(|pid| by_id.get(&pid)) {
            Some(parent) => format!("{} / {}", parent, name),
            None => name.clone(),
        };
        CategoryRow { id, name, display_name, parent_id, color: color.unwrap_or_default(), active }
    }).collect();

    // Labels with use counts
    let lab_rows: Vec<(i64, String, Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT l.id, l.name, l.color, l.active, COALESCE(COUNT(el.expense_id), 0)
         FROM labels l LEFT JOIN expense_labels el ON el.label_id = l.id
         GROUP BY l.id ORDER BY l.active DESC, l.name",
    ).fetch_all(&state.pool).await?;
    let labels: Vec<LabelRow> = lab_rows.into_iter()
        .map(|(id, name, color, active, use_count)| LabelRow {
            id, name, color: color.unwrap_or_default(), active, use_count,
        })
        .collect();

    // Expenses with category + labels (LEFT JOIN; aggregate label IDs/names per row).
    // Filtered by year when set.
    let exp_rows: Vec<(i64, String, f64, Option<String>, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT e.id, e.as_of, e.amount_usd * 1.0, e.place, e.notes, e.category_id, c.name,
                GROUP_CONCAT(el.label_id),
                GROUP_CONCAT(l.name, ', ')
         FROM expenses e
         LEFT JOIN categories c ON c.id = e.category_id
         LEFT JOIN expense_labels el ON el.expense_id = e.id
         LEFT JOIN labels l ON l.id = el.label_id
         WHERE e.as_of >= ?1 AND e.as_of <= ?2
         GROUP BY e.id ORDER BY e.as_of DESC LIMIT 240",
    ).bind(&year_lo).bind(&year_hi).fetch_all(&state.pool).await?;
    let expense_rows: Vec<ExpenseRow> = exp_rows.into_iter()
        .map(|(id, as_of, amount_usd, place, notes, cat_id, cat_name, lab_ids, lab_names)| ExpenseRow {
            id, as_of, amount_usd,
            selected_category_id: cat_id.unwrap_or(0),
            place: place.unwrap_or_default(),
            notes: notes.unwrap_or_default(),
            category_id: cat_id,
            category_name: cat_name.unwrap_or_default(),
            label_ids_csv: lab_ids.unwrap_or_default(),
            labels_display: lab_names.unwrap_or_default(),
        })
        .collect();

    // Targets — grouped by category with bull/crab/bear columns
    let tgt_rows: Vec<(String, String, f64)> = sqlx::query_as(
        "SELECT category, market_mode, target_pct FROM allocation_targets ORDER BY category, market_mode",
    ).fetch_all(&state.pool).await?;
    let mut group_map: std::collections::BTreeMap<String, (f64, f64, f64)> = std::collections::BTreeMap::new();
    for (cat, mode, pct) in tgt_rows {
        let entry = group_map.entry(cat).or_insert((0.0, 0.0, 0.0));
        match mode.as_str() {
            "bull" => entry.0 = pct,
            "crab" => entry.1 = pct,
            "bear" => entry.2 = pct,
            _ => {}
        }
    }
    let target_groups: Vec<TargetGroupRow> = group_map.into_iter()
        .map(|(category, (b, c, br))| TargetGroupRow { category, bull_pct: b, crab_pct: c, bear_pct: br })
        .collect();
    // Per-mode column totals — server-rendered as the initial Σ row; the
    // targets template's inline JS recomputes live on input changes.
    let target_total_bull = target_groups.iter().map(|g| g.bull_pct).sum::<f64>();
    let target_total_crab = target_groups.iter().map(|g| g.crab_pct).sum::<f64>();
    let target_total_bear = target_groups.iter().map(|g| g.bear_pct).sum::<f64>();

    Ok(DataTemplate {
        is_assets: tab == "assets",
        is_snapshots: tab == "snapshots",
        is_positions: tab == "positions",
        is_accounts: tab == "accounts",
        is_income: tab == "income",
        is_expenses: tab == "expenses",
        is_targets: tab == "targets",
        is_categories: tab == "categories",
        is_labels: tab == "labels",
        assets, snapshots, positions, accounts,
        income_rows, expense_rows, target_groups,
        target_total_bull, target_total_crab, target_total_bear,
        categories, labels,
        snapshot_dates, selected_date,
        year_all_selected, income_year_options, expense_year_options,
    })
}
