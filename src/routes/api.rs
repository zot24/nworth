//! JSON API endpoints consumed by Chart.js.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;

use crate::{error::AppError, AppState};

// ---------- Shared types ----------

#[derive(Serialize)]
pub struct SeriesPoint {
    pub as_of: String,
    pub value_usd: f64,
}

#[derive(Serialize)]
pub struct TypeSeries {
    pub type_code: String,
    pub points: Vec<SeriesPoint>,
}

#[derive(Serialize)]
pub struct NetWorthSeries {
    pub points: Vec<SeriesPoint>,
    pub by_type: Vec<TypeSeries>,
}

#[derive(Serialize)]
pub struct AllocationSlice {
    pub symbol: String,
    pub type_code: String,
    pub value_usd: f64,
    pub role: String,
}

// ---------- Dashboard APIs ----------

/// GET /api/networth
pub async fn networth_series(
    State(state): State<AppState>,
) -> Result<Json<NetWorthSeries>, AppError> {
    let total: Vec<(String, f64)> = sqlx::query_as(
        "SELECT as_of, SUM(value_usd) AS v
         FROM snapshots GROUP BY as_of ORDER BY as_of",
    )
    .fetch_all(&state.pool)
    .await?;

    let points = total
        .into_iter()
        .map(|(as_of, value_usd)| SeriesPoint { as_of, value_usd })
        .collect();

    let per_type: Vec<(String, String, f64)> = sqlx::query_as(
        "SELECT s.as_of, a.type_code, SUM(s.value_usd)
         FROM snapshots s JOIN assets a ON a.id = s.asset_id
         GROUP BY s.as_of, a.type_code ORDER BY s.as_of, a.type_code",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut by_type_map: std::collections::BTreeMap<String, Vec<SeriesPoint>> =
        std::collections::BTreeMap::new();
    for (as_of, type_code, v) in per_type {
        by_type_map
            .entry(type_code)
            .or_default()
            .push(SeriesPoint { as_of, value_usd: v });
    }
    let by_type = by_type_map
        .into_iter()
        .map(|(type_code, points)| TypeSeries { type_code, points })
        .collect();

    Ok(Json(NetWorthSeries { points, by_type }))
}

/// GET /api/allocation
pub async fn allocation(
    State(state): State<AppState>,
) -> Result<Json<Vec<AllocationSlice>>, AppError> {
    let latest: String =
        sqlx::query_scalar("SELECT COALESCE(MAX(as_of), '') FROM snapshots")
            .fetch_one(&state.pool)
            .await?;

    if latest.is_empty() {
        return Ok(Json(vec![]));
    }

    // Group also by role so the client can split rows into the right donut
    // category (a single symbol could in theory appear under multiple roles).
    let rows: Vec<(String, String, String, f64)> = sqlx::query_as(
        "SELECT a.symbol, a.type_code, ac.role, SUM(s.value_usd)
         FROM snapshots s
         JOIN assets a   ON a.id = s.asset_id
         JOIN accounts ac ON ac.id = s.account_id
         WHERE s.as_of = ?1
         GROUP BY a.symbol, a.type_code, ac.role
         HAVING SUM(s.value_usd) > 0
         ORDER BY 4 DESC",
    )
    .bind(&latest)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(symbol, type_code, role, value_usd)| AllocationSlice {
                symbol,
                type_code,
                value_usd,
                role,
            })
            .collect(),
    ))
}

// ---------- Stocks APIs ----------

#[derive(Serialize)]
pub struct StockHistoryPoint {
    pub as_of: String,
    pub account_name: String,
    pub value_usd: f64,
}

/// GET /api/stocks/history
pub async fn stocks_history(
    State(state): State<AppState>,
) -> Result<Json<Vec<StockHistoryPoint>>, AppError> {
    let rows: Vec<(String, String, f64)> = sqlx::query_as(
        "SELECT s.as_of, ac.name, SUM(s.value_usd)
         FROM snapshots s
         JOIN accounts ac ON ac.id = s.account_id
         JOIN assets a ON a.id = s.asset_id
         WHERE a.type_code = 'stock'
         GROUP BY s.as_of, ac.name
         ORDER BY s.as_of",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(as_of, account_name, value_usd)| StockHistoryPoint {
                as_of,
                account_name,
                value_usd,
            })
            .collect(),
    ))
}

#[derive(Serialize)]
pub struct DividendPoint {
    pub quarter: String,
    pub value_usd: f64,
}

/// GET /api/stocks/dividends
pub async fn stocks_dividends(
    State(state): State<AppState>,
) -> Result<Json<Vec<DividendPoint>>, AppError> {
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT SUBSTR(ts, 1, 4) || '-Q' ||
                CASE
                    WHEN CAST(SUBSTR(ts, 6, 2) AS INTEGER) <= 3 THEN '1'
                    WHEN CAST(SUBSTR(ts, 6, 2) AS INTEGER) <= 6 THEN '2'
                    WHEN CAST(SUBSTR(ts, 6, 2) AS INTEGER) <= 9 THEN '3'
                    ELSE '4'
                END AS quarter,
                SUM(quantity) AS total
         FROM transactions
         WHERE kind = 'dividend'
         GROUP BY quarter
         ORDER BY quarter",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(quarter, value_usd)| DividendPoint { quarter, value_usd })
            .collect(),
    ))
}

// ---------- Crypto API ----------

#[derive(Serialize)]
pub struct CryptoHistoryPoint {
    pub as_of: String,
    pub symbol: String,
    pub value_usd: f64,
}

/// GET /api/crypto/history
pub async fn crypto_history(
    State(state): State<AppState>,
) -> Result<Json<Vec<CryptoHistoryPoint>>, AppError> {
    let rows: Vec<(String, String, f64)> = sqlx::query_as(
        "SELECT s.as_of, a.symbol, SUM(s.value_usd)
         FROM snapshots s
         JOIN assets a ON a.id = s.asset_id
         WHERE a.type_code = 'crypto'
         GROUP BY s.as_of, a.symbol
         ORDER BY s.as_of",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(as_of, symbol, value_usd)| CryptoHistoryPoint {
                as_of,
                symbol,
                value_usd,
            })
            .collect(),
    ))
}

// ---------- Cash API ----------

#[derive(Serialize)]
pub struct CashHistoryPoint {
    pub as_of: String,
    pub symbol: String,
    pub value_usd: f64,
}

/// GET /api/cash/history
pub async fn cash_history(
    State(state): State<AppState>,
) -> Result<Json<Vec<CashHistoryPoint>>, AppError> {
    let rows: Vec<(String, String, f64)> = sqlx::query_as(
        "SELECT s.as_of, a.symbol, SUM(s.value_usd)
         FROM snapshots s
         JOIN assets a ON a.id = s.asset_id
         WHERE a.type_code IN ('fiat', 'stable')
         GROUP BY s.as_of, a.symbol
         ORDER BY s.as_of",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(as_of, symbol, value_usd)| CashHistoryPoint {
                as_of,
                symbol,
                value_usd,
            })
            .collect(),
    ))
}

// ---------- Account detail API ----------

#[derive(Serialize)]
pub struct AccountHistoryPoint {
    pub as_of: String,
    pub symbol: String,
    pub value_usd: f64,
}

/// GET /api/accounts/:id/history
pub async fn account_history(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<AccountHistoryPoint>>, AppError> {
    let rows: Vec<(String, String, f64)> = sqlx::query_as(
        "SELECT s.as_of, a.symbol, SUM(s.value_usd)
         FROM snapshots s
         JOIN assets a ON a.id = s.asset_id
         WHERE s.account_id = ?1
         GROUP BY s.as_of, a.symbol
         ORDER BY s.as_of",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(as_of, symbol, value_usd)| AccountHistoryPoint {
                as_of,
                symbol,
                value_usd,
            })
            .collect(),
    ))
}

// ---------- Financial APIs ----------

#[derive(Serialize)]
pub struct IncomeMonthly {
    pub as_of: String,
    pub salary_usd: f64,
    pub bonus_usd: f64,
    pub taxes_usd: f64,
}

/// GET /api/income/monthly
pub async fn income_monthly(
    State(state): State<AppState>,
) -> Result<Json<Vec<IncomeMonthly>>, AppError> {
    let rows: Vec<(String, f64, f64, f64)> = sqlx::query_as(
        "SELECT as_of, salary_usd, bonus_usd, taxes_usd
         FROM income ORDER BY as_of",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(as_of, salary_usd, bonus_usd, taxes_usd)| IncomeMonthly {
                as_of,
                salary_usd,
                bonus_usd,
                taxes_usd,
            })
            .collect(),
    ))
}

#[derive(Serialize)]
pub struct ExpenseMonthly {
    pub as_of: String,
    pub amount_usd: f64,
    /// Optional location/group label (e.g. "South America", "Argentina") so
    /// the chart tooltip can show context above the dollar amount.
    pub place: Option<String>,
}

/// GET /api/expenses/monthly
pub async fn expenses_monthly(
    State(state): State<AppState>,
) -> Result<Json<Vec<ExpenseMonthly>>, AppError> {
    let rows: Vec<(String, f64, Option<String>)> = sqlx::query_as(
        "SELECT as_of, amount_usd, place FROM expenses ORDER BY as_of",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(as_of, amount_usd, place)| ExpenseMonthly { as_of, amount_usd, place })
            .collect(),
    ))
}

#[derive(Serialize)]
pub struct ExpenseCategorySlice {
    pub category_id: Option<i64>,
    pub category_name: String,
    pub color: Option<String>,
    pub amount_usd: f64,
    pub count: i64,
}

/// GET /api/expenses/by-category?window=month|quarter|ytd|all
/// Returns expense totals grouped by category for the given trailing window.
pub async fn expenses_by_category(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<ExpenseCategorySlice>>, AppError> {
    let window = q.get("window").map(|s| s.as_str()).unwrap_or("month");
    let cutoff = match window {
        "quarter" => "date('now', '-90 days')",
        "ytd"     => "date(strftime('%Y', 'now') || '-01-01')",
        "all"     => "date('1970-01-01')",
        _ /* month */ => "date('now', '-30 days')",
    };
    let sql = format!(
        "SELECT e.category_id, COALESCE(c.name, '(uncategorized)'), c.color,
                SUM(e.amount_usd) * 1.0, COUNT(*)
         FROM expenses e LEFT JOIN categories c ON c.id = e.category_id
         WHERE e.as_of >= {cutoff}
         GROUP BY e.category_id
         ORDER BY SUM(e.amount_usd) DESC",
    );
    let rows: Vec<(Option<i64>, String, Option<String>, f64, i64)> = sqlx::query_as(&sql)
        .fetch_all(&state.pool).await?;
    Ok(Json(rows.into_iter().map(|(category_id, category_name, color, amount_usd, count)| ExpenseCategorySlice {
        category_id, category_name, color, amount_usd, count,
    }).collect()))
}

#[derive(Serialize)]
pub struct FlowMonthly {
    pub as_of: String,
    pub income: f64,
    pub expenses: f64,
    pub savings: f64,
}

/// GET /api/flow/monthly
pub async fn flow_monthly(
    State(state): State<AppState>,
) -> Result<Json<Vec<FlowMonthly>>, AppError> {
    let rows: Vec<(String, f64, f64)> = sqlx::query_as(
        "SELECT m.as_of,
                COALESCE(i.net * 1.0, 0.0),
                COALESCE(e.amount_usd * 1.0, 0.0)
         FROM (SELECT as_of FROM income UNION SELECT as_of FROM expenses) m
         LEFT JOIN (SELECT as_of, (salary_usd + bonus_usd - taxes_usd) * 1.0 AS net FROM income) i
           ON i.as_of = m.as_of
         LEFT JOIN expenses e ON e.as_of = m.as_of
         ORDER BY m.as_of",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(as_of, income, expenses)| FlowMonthly {
                as_of,
                income,
                expenses,
                savings: income - expenses,
            })
            .collect(),
    ))
}

// ---------- Dashboard: Net worth by category ----------

#[derive(Serialize)]
pub struct CategoryPoint {
    pub as_of: String,
    pub category: String,
    pub value_usd: f64,
}

/// GET /api/networth/by-category
/// Maps type_code to display categories: stock→Stocks, stable→Stable Yielding, crypto/nft→Crypto, fiat→Cash.
/// Operating-role accounts collapse into "Operating", property-role into "Property".
pub async fn networth_by_category(
    State(state): State<AppState>,
) -> Result<Json<Vec<CategoryPoint>>, AppError> {
    let rows: Vec<(String, String, String, f64)> = sqlx::query_as(
        "SELECT s.as_of, a.type_code, ac.role, SUM(s.value_usd)
         FROM snapshots s
         JOIN assets a ON a.id = s.asset_id
         JOIN accounts ac ON ac.id = s.account_id
         GROUP BY s.as_of, a.type_code, ac.role
         ORDER BY s.as_of",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut cat_map: std::collections::BTreeMap<(String, String), f64> =
        std::collections::BTreeMap::new();
    for (as_of, type_code, role, val) in rows {
        let category = match role.as_str() {
            "operating" => "Operating".to_string(),
            "property" => "Property".to_string(),
            _ => match type_code.as_str() {
                "stock" => "Stocks".to_string(),
                "stable" => "Stable Yielding".to_string(),
                "crypto" | "nft" => "Crypto".to_string(),
                "fiat" => "Cash".to_string(),
                other => other.to_string(),
            },
        };
        *cat_map.entry((as_of, category)).or_default() += val;
    }

    Ok(Json(
        cat_map
            .into_iter()
            .map(|((as_of, category), value_usd)| CategoryPoint {
                as_of,
                category,
                value_usd,
            })
            .collect(),
    ))
}

// ---------- Dashboard: Allocation doughnuts ----------

#[derive(Serialize)]
pub struct HoldingSlice {
    pub symbol: String,
    pub value_usd: f64,
    pub pct: f64,
}

/// GET /api/allocation/stocks — individual stock holdings for doughnut
pub async fn allocation_stocks(
    State(state): State<AppState>,
) -> Result<Json<Vec<HoldingSlice>>, AppError> {
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT a.symbol, p.quantity * COALESCE(p.avg_cost, 0) * 1.0
         FROM positions p
         JOIN assets a ON a.id = p.asset_id
         WHERE a.type_code = 'stock' AND a.symbol != 'STOCKS_TOTAL'
         ORDER BY 2 DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    let total: f64 = rows.iter().map(|(_, v)| v).sum();
    Ok(Json(
        rows.into_iter()
            .filter(|(_, v)| *v > 0.0)
            .map(|(symbol, value_usd)| HoldingSlice {
                symbol,
                pct: if total > 0.0 { value_usd / total * 100.0 } else { 0.0 },
                value_usd,
            })
            .collect(),
    ))
}

/// GET /api/allocation/crypto — individual crypto token holdings for doughnut
pub async fn allocation_crypto(
    State(state): State<AppState>,
) -> Result<Json<Vec<HoldingSlice>>, AppError> {
    let latest: String =
        sqlx::query_scalar("SELECT COALESCE(MAX(as_of), '') FROM snapshots")
            .fetch_one(&state.pool)
            .await?;

    if latest.is_empty() {
        return Ok(Json(vec![]));
    }

    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT a.symbol, SUM(s.value_usd)
         FROM snapshots s
         JOIN assets a ON a.id = s.asset_id
         WHERE s.as_of = ?1 AND a.type_code IN ('crypto', 'nft')
         GROUP BY a.symbol
         HAVING SUM(s.value_usd) > 0
         ORDER BY 2 DESC",
    )
    .bind(&latest)
    .fetch_all(&state.pool)
    .await?;

    let total: f64 = rows.iter().map(|(_, v)| v).sum();
    Ok(Json(
        rows.into_iter()
            .map(|(symbol, value_usd)| HoldingSlice {
                symbol,
                pct: if total > 0.0 { value_usd / total * 100.0 } else { 0.0 },
                value_usd,
            })
            .collect(),
    ))
}

// ---------- Stocks: Dividend analysis ----------

#[derive(Serialize)]
pub struct MonthlyDividend {
    pub as_of: String,
    pub value_usd: f64,
}

/// GET /api/stocks/dividends/monthly
pub async fn dividends_monthly(
    State(state): State<AppState>,
) -> Result<Json<Vec<MonthlyDividend>>, AppError> {
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT ts, SUM(quantity) FROM transactions
         WHERE kind = 'dividend'
         GROUP BY ts ORDER BY ts",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(as_of, value_usd)| MonthlyDividend { as_of, value_usd })
            .collect(),
    ))
}

#[derive(Serialize)]
pub struct YearlyDividend {
    pub year: String,
    pub value_usd: f64,
    pub growth_pct: Option<f64>,
}

/// GET /api/stocks/dividends/yearly — with YoY growth %
pub async fn dividends_yearly(
    State(state): State<AppState>,
) -> Result<Json<Vec<YearlyDividend>>, AppError> {
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT SUBSTR(ts, 1, 4), SUM(quantity)
         FROM transactions WHERE kind = 'dividend'
         GROUP BY SUBSTR(ts, 1, 4) ORDER BY 1",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut result = Vec::with_capacity(rows.len());
    let mut prev: Option<f64> = None;
    for (year, value_usd) in rows {
        let growth_pct = prev.map(|p| if p > 0.0 { (value_usd - p) / p * 100.0 } else { 0.0 });
        result.push(YearlyDividend { year, value_usd, growth_pct });
        prev = Some(value_usd);
    }

    Ok(Json(result))
}

#[derive(Serialize)]
pub struct YoyDividend {
    pub month: String,
    pub year: String,
    pub value_usd: f64,
}

/// GET /api/stocks/dividends/yoy — year-over-year comparison by month
pub async fn dividends_yoy(
    State(state): State<AppState>,
) -> Result<Json<Vec<YoyDividend>>, AppError> {
    let rows: Vec<(String, String, f64)> = sqlx::query_as(
        "SELECT SUBSTR(ts, 6, 2) AS month, SUBSTR(ts, 1, 4) AS year, SUM(quantity)
         FROM transactions WHERE kind = 'dividend'
         GROUP BY month, year ORDER BY month, year",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(month, year, value_usd)| YoyDividend { month, year, value_usd })
            .collect(),
    ))
}

// ---------- Stocks: Holdings & Growth ----------

#[derive(Serialize)]
pub struct StockHolding {
    pub symbol: String,
    pub account_name: String,
    pub quantity: f64,
    pub avg_cost: f64,
    pub market_value: f64,
    pub gain_loss: f64,
    pub pct: f64,
}

/// GET /api/stocks/holdings — current positions with P&L
pub async fn stocks_holdings(
    State(state): State<AppState>,
) -> Result<Json<Vec<StockHolding>>, AppError> {
    let rows: Vec<(String, String, f64, f64, f64)> = sqlx::query_as(
        "SELECT a.symbol, ac.name,
                p.quantity * 1.0,
                COALESCE(p.avg_cost * 1.0, 0.0),
                COALESCE(p.last_price * 1.0, 0.0)
         FROM positions p
         JOIN assets a ON a.id = p.asset_id
         JOIN accounts ac ON ac.id = p.account_id
         WHERE a.type_code = 'stock' AND a.symbol != 'STOCKS_TOTAL'
         ORDER BY p.quantity * COALESCE(p.avg_cost, 0) DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    let holdings: Vec<StockHolding> = rows
        .iter()
        .map(|(symbol, account_name, qty, avg_cost, _last)| {
            let market_value = qty * avg_cost; // Use cost basis as proxy until live prices
            StockHolding {
                symbol: symbol.clone(),
                account_name: account_name.clone(),
                quantity: *qty,
                avg_cost: *avg_cost,
                market_value,
                gain_loss: 0.0, // Requires live prices
                pct: 0.0,
            }
        })
        .collect();

    let total: f64 = holdings.iter().map(|h| h.market_value).sum();
    let holdings = holdings
        .into_iter()
        .map(|mut h| {
            h.pct = if total > 0.0 { h.market_value / total * 100.0 } else { 0.0 };
            h
        })
        .collect();

    Ok(Json(holdings))
}

#[derive(Serialize)]
pub struct YearlyGrowth {
    pub year: String,
    pub start_value: f64,
    pub end_value: f64,
    pub growth_pct: f64,
}

/// GET /api/stocks/growth — yearly portfolio value growth
pub async fn stocks_growth(
    State(state): State<AppState>,
) -> Result<Json<Vec<YearlyGrowth>>, AppError> {
    // Get first and last snapshot value per year for stocks
    let rows: Vec<(String, f64, f64)> = sqlx::query_as(
        "SELECT year, MIN(val) AS start_val, MAX(val) AS end_val FROM (
            SELECT SUBSTR(s.as_of, 1, 4) AS year,
                   s.as_of,
                   SUM(s.value_usd) AS val
            FROM snapshots s
            JOIN assets a ON a.id = s.asset_id
            WHERE a.type_code = 'stock'
            GROUP BY s.as_of
        )
        GROUP BY year ORDER BY year",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(year, start_value, end_value)| {
                let growth_pct = if start_value > 0.0 {
                    (end_value - start_value) / start_value * 100.0
                } else {
                    0.0
                };
                YearlyGrowth { year, start_value, end_value, growth_pct }
            })
            .collect(),
    ))
}

// ---------- Pivot table calculations ----------

#[derive(Serialize)]
pub struct AdjustmentSlice {
    pub category: String,
    pub current_value: f64,
    pub current_pct: f64,
    pub target_pct: f64,
    pub target_value: f64,
    pub adjustment: f64, // positive = buy, negative = sell
}

/// GET /api/allocation/adjustments — how much to buy/sell per category to hit targets
pub async fn allocation_adjustments(
    State(state): State<AppState>,
) -> Result<Json<Vec<AdjustmentSlice>>, AppError> {
    let latest: String =
        sqlx::query_scalar("SELECT COALESCE(MAX(as_of), '') FROM snapshots")
            .fetch_one(&state.pool)
            .await?;

    if latest.is_empty() {
        return Ok(Json(vec![]));
    }

    // Current value by category — investment-role accounts only. Operating + property
    // roles aren't tradable, so they don't have allocation targets and shouldn't skew
    // the denominator.
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT a.type_code, SUM(s.value_usd) * 1.0
         FROM snapshots s
         JOIN assets a   ON a.id = s.asset_id
         JOIN accounts ac ON ac.id = s.account_id
         WHERE s.as_of = ?1 AND ac.role = 'investment'
         GROUP BY a.type_code",
    )
    .bind(&latest)
    .fetch_all(&state.pool)
    .await?;

    let mut cat_values: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for (type_code, val) in &rows {
        let category = match type_code.as_str() {
            "stock" => "stocks".to_string(),
            "stable" => "stable_yielding".to_string(),
            "crypto" | "nft" => "crypto".to_string(),
            "fiat" => "cash".to_string(),
            other => other.to_string(),
        };
        *cat_values.entry(category).or_default() += val;
    }

    let total: f64 = cat_values.values().sum();

    // Targets
    let targets: Vec<(String, f64)> = sqlx::query_as(
        "SELECT category, target_pct FROM allocation_targets",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut result: Vec<AdjustmentSlice> = targets
        .into_iter()
        .map(|(category, target_pct)| {
            let current_value = cat_values.get(&category).copied().unwrap_or(0.0);
            let current_pct = if total > 0.0 { current_value / total } else { 0.0 };
            let target_value = total * target_pct;
            let adjustment = target_value - current_value;
            AdjustmentSlice {
                category,
                current_value,
                current_pct,
                target_pct,
                target_value,
                adjustment,
            }
        })
        .collect();
    result.sort_by(|a, b| b.current_value.partial_cmp(&a.current_value).unwrap());
    Ok(Json(result))
}

#[derive(Serialize)]
pub struct ApyInfo {
    pub total_stables: f64,
    pub apy_pct: f64,
    pub annual_income: f64,
    pub monthly_income: f64,
}

/// GET /api/stables/apy — Stable Yielding APY calculation
pub async fn stables_apy(
    State(state): State<AppState>,
) -> Result<Json<ApyInfo>, AppError> {
    let latest: String =
        sqlx::query_scalar("SELECT COALESCE(MAX(as_of), '') FROM snapshots")
            .fetch_one(&state.pool)
            .await?;

    let total_stables: f64 = if latest.is_empty() {
        0.0
    } else {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(s.value_usd) * 1.0, 0.0)
             FROM snapshots s JOIN assets a ON a.id = s.asset_id
             WHERE s.as_of = ?1 AND a.type_code = 'stable'",
        )
        .bind(&latest)
        .fetch_one(&state.pool)
        .await?
    };

    // Default 6% APY (from xlsx Overall Pivot Table)
    let apy_pct = 6.0;
    let annual_income = total_stables * apy_pct / 100.0;
    let monthly_income = annual_income / 12.0;

    Ok(Json(ApyInfo {
        total_stables,
        apy_pct,
        annual_income,
        monthly_income,
    }))
}

#[derive(Serialize)]
pub struct NormalizedHolding {
    pub symbol: String,
    pub original_symbol: String,
    pub account_name: String,
    pub quantity: f64,
    pub normalized_quantity: f64,
    pub avg_cost: f64,
    pub market_value: f64,
}

/// GET /api/stocks/normalized — cross-fund cost-basis normalization for comparison
/// Converts alternate-share-class holdings (e.g. FXAIX) to a reference-fund equivalent
/// (VOO) using the cost-basis ratio so they can be compared on the same axis.
pub async fn stocks_normalized(
    State(state): State<AppState>,
) -> Result<Json<Vec<NormalizedHolding>>, AppError> {
    let rows: Vec<(String, String, f64, f64)> = sqlx::query_as(
        "SELECT a.symbol, ac.name, p.quantity * 1.0, COALESCE(p.avg_cost * 1.0, 0.0)
         FROM positions p
         JOIN assets a ON a.id = p.asset_id
         JOIN accounts ac ON ac.id = p.account_id
         WHERE a.type_code = 'stock' AND a.symbol != 'STOCKS_TOTAL'
         ORDER BY p.quantity * COALESCE(p.avg_cost, 0) DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    // Find VOO avg cost for normalization
    let voo_avg: f64 = rows.iter()
        .filter(|(s, _, _, _)| s == "VOO")
        .map(|(_, _, _, c)| *c)
        .next()
        .unwrap_or(500.0);

    let holdings: Vec<NormalizedHolding> = rows
        .into_iter()
        .map(|(symbol, account_name, quantity, avg_cost)| {
            let market_value = quantity * avg_cost;
            // Normalize: convert cost basis to VOO-equivalent shares
            let normalized_quantity = if voo_avg > 0.0 {
                market_value / voo_avg
            } else {
                quantity
            };
            NormalizedHolding {
                original_symbol: symbol.clone(),
                symbol: if symbol == "FXAIX" { "VOO (eq)".to_string() } else { symbol },
                account_name,
                quantity,
                normalized_quantity,
                avg_cost,
                market_value,
            }
        })
        .collect();

    Ok(Json(holdings))
}

// ---------- Market Sentiment ----------

#[derive(Serialize)]
pub struct PortfolioMomentum {
    pub change_3m_pct: f64,
    pub change_6m_pct: f64,
    pub trend: String,
}

#[derive(Serialize)]
pub struct MarketSignal {
    pub status: String,        // "active" or "insufficient_data"
    pub days_of_data: i64,
    pub days_needed: i64,
    pub ma_50: Option<f64>,
    pub ma_200: Option<f64>,
    pub current_price: Option<f64>,
    pub signal: String,        // "bull", "bear", "crab", "pending"
    pub label: String,
}

#[derive(Serialize)]
pub struct SectorSentiment {
    pub category: String,
    pub representative_asset: Option<String>,
    pub portfolio_momentum: PortfolioMomentum,
    pub market_signal: MarketSignal,
}

/// GET /api/market/sentiment
pub async fn market_sentiment(
    State(state): State<AppState>,
) -> Result<Json<Vec<SectorSentiment>>, AppError> {
    // Representative assets per category for MA calculation
    let rep_assets: Vec<(&str, &str, &str)> = vec![
        ("stocks", "VOO", "stock"),
        ("crypto", "BTC", "crypto"),
    ];

    // Portfolio momentum from snapshots. Non-investment roles (operating + property)
    // have no market sentiment — exclude them so they don't pollute the per-category
    // MA signal.
    let snap_rows: Vec<(String, String, f64)> = sqlx::query_as(
        "SELECT
            CASE
                WHEN a.type_code = 'stock' THEN 'stocks'
                WHEN a.type_code = 'stable' THEN 'stable_yielding'
                WHEN a.type_code IN ('crypto','nft') THEN 'crypto'
                WHEN a.type_code = 'fiat' THEN 'cash'
                ELSE a.type_code
            END AS category,
            s.as_of,
            SUM(s.value_usd) * 1.0
         FROM snapshots s
         JOIN assets a ON a.id = s.asset_id
         JOIN accounts ac ON ac.id = s.account_id
         WHERE ac.role = 'investment'
         GROUP BY category, s.as_of
         ORDER BY category, s.as_of",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut by_cat: std::collections::HashMap<String, Vec<(String, f64)>> =
        std::collections::HashMap::new();
    for (cat, date, val) in snap_rows {
        by_cat.entry(cat).or_default().push((date, val));
    }

    // Build MA signals from price_history
    let mut ma_signals: std::collections::HashMap<String, MarketSignal> =
        std::collections::HashMap::new();

    for (category, symbol, type_code) in &rep_assets {
        // Get price history for this asset
        let prices: Vec<(f64,)> = sqlx::query_as(
            "SELECT ph.price_usd * 1.0
             FROM price_history ph
             JOIN assets a ON a.id = ph.asset_id
             WHERE a.symbol = ?1 AND a.type_code = ?2
             ORDER BY ph.as_of ASC",
        )
        .bind(symbol)
        .bind(type_code)
        .fetch_all(&state.pool)
        .await?;

        let n = prices.len() as i64;
        let vals: Vec<f64> = prices.into_iter().map(|(p,)| p).collect();

        if n < 50 {
            ma_signals.insert(category.to_string(), MarketSignal {
                status: "insufficient_data".to_string(),
                days_of_data: n,
                days_needed: 200,
                ma_50: None,
                ma_200: None,
                current_price: vals.last().copied(),
                signal: "pending".to_string(),
                label: format!("Collecting data — {n}/200 days of {symbol} prices"),
            });
        } else {
            let current = *vals.last().unwrap();
            let ma50: f64 = vals[vals.len()-50..].iter().sum::<f64>() / 50.0;
            let ma200: f64 = if n >= 200 {
                vals[vals.len()-200..].iter().sum::<f64>() / 200.0
            } else {
                vals.iter().sum::<f64>() / vals.len() as f64
            };

            let (signal, label) = if current > ma200 && ma50 > ma200 {
                ("bull".to_string(), format!("Bull — {symbol} above 200MA, Golden Cross"))
            } else if current < ma200 && ma50 < ma200 {
                ("bear".to_string(), format!("Bear — {symbol} below 200MA, Death Cross"))
            } else {
                ("crab".to_string(), format!("Crab — {symbol} mixed signals (50/200 MA)"))
            };

            ma_signals.insert(category.to_string(), MarketSignal {
                status: if n >= 200 { "active" } else { "partial" }.to_string(),
                days_of_data: n,
                days_needed: 200,
                ma_50: Some(ma50),
                ma_200: Some(ma200),
                current_price: Some(current),
                signal,
                label,
            });
        }
    }

    // Combine portfolio momentum + market signals
    let mut results = Vec::new();
    let categories = ["stocks", "crypto", "stable_yielding", "cash"];

    for cat in categories {
        let points = by_cat.get(cat);
        let momentum = if let Some(pts) = points {
            let n = pts.len();
            if n == 0 {
                PortfolioMomentum { change_3m_pct: 0.0, change_6m_pct: 0.0, trend: "crab".into() }
            } else {
                let current = pts[n - 1].1;
                let val_3m = if n > 3 { pts[n - 4].1 } else { pts[0].1 };
                let val_6m = if n > 6 { pts[n - 7].1 } else { pts[0].1 };
                let chg3 = if val_3m > 0.0 { (current - val_3m) / val_3m * 100.0 } else { 0.0 };
                let chg6 = if val_6m > 0.0 { (current - val_6m) / val_6m * 100.0 } else { 0.0 };
                let trend = if chg3 > 10.0 { "bull" } else if chg3 < -10.0 { "bear" } else { "crab" };
                PortfolioMomentum { change_3m_pct: chg3, change_6m_pct: chg6, trend: trend.into() }
            }
        } else {
            PortfolioMomentum { change_3m_pct: 0.0, change_6m_pct: 0.0, trend: "crab".into() }
        };

        let market = ma_signals.remove(cat).unwrap_or(MarketSignal {
            status: "not_applicable".to_string(),
            days_of_data: 0, days_needed: 0,
            ma_50: None, ma_200: None, current_price: None,
            signal: "crab".to_string(),
            label: format!("{cat} — no market indicator (stable/fiat)"),
        });

        let rep = rep_assets.iter().find(|(c, _, _)| *c == cat).map(|(_, s, _)| s.to_string());

        results.push(SectorSentiment {
            category: cat.to_string(),
            representative_asset: rep,
            portfolio_momentum: momentum,
            market_signal: market,
        });
    }

    Ok(Json(results))
}
