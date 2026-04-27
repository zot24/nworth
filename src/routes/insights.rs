//! Decision-making analytics for the Action Center.
//!
//! Computes drift from active targets, concentration risk, net-worth deltas, and
//! a ranked list of recommended actions with an urgency score.
//!
//! Endpoints:
//!   GET /api/insights/summary    — single bundled payload for the homepage
//!   GET /api/insights/drift      — per-category drift (pp + USD) under active mode
//!   GET /api/insights/concentration — top1 / top3 / HHI
//!   GET /api/insights/networth/deltas — 7d / 30d / YTD
//!   GET /api/insights/actions    — ranked action list

use axum::{extract::State, Json};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;

use crate::{error::AppError, AppState};

// Tunables — see specs from the financial-engineer + portfolio-manager passes.
const DRIFT_MIN_PP: f64 = 2.0;          // suppress alert below this magnitude
const DRIFT_MIN_USD: f64 = 1_000.0;     // and below this dollar amount
const MIN_TRADE_SIZE_FRAC: f64 = 0.005; // 0.5% of NW
const MIN_TRADE_SIZE_USD: f64 = 500.0;  // floor
const CONCENTRATION_TOP1_THRESHOLD: f64 = 0.25;
const STALE_SNAPSHOT_DAYS: i64 = 14;
const STALE_OWNED_DAYS: i64 = 90;       // owned-asset snapshots get a refresh nudge after this

const URGENT_SCORE: f64 = 70.0;
const SUGGESTED_SCORE: f64 = 35.0;

// ────────── Drift ──────────

#[derive(Serialize, Clone)]
pub struct CategoryDrift {
    pub category: String,
    pub current_value: f64,
    pub current_pct: f64,
    pub target_pct: f64,
    pub drift_pp: f64,        // signed: positive = over target
    pub drift_usd: f64,       // signed: positive = over target (sell to fix)
    pub adjustment_usd: f64,  // signed: positive = buy, negative = sell (mirror of drift_usd)
    pub tradable: bool,
}

#[derive(Serialize)]
pub struct DriftReport {
    pub total_value: f64,
    pub tradable_value: f64,    // investment accounts only — drift denominator
    pub owned_value: f64,       // is_investment=0 accounts (car, future house, etc.)
    pub active_mode: String,
    pub abs_drift_pp: f64,
    pub categories: Vec<CategoryDrift>,
}

pub async fn drift(State(s): State<AppState>) -> Result<Json<DriftReport>, AppError> {
    let report = compute_drift(&s.pool).await?;
    Ok(Json(report))
}

async fn compute_drift(pool: &SqlitePool) -> Result<DriftReport, AppError> {
    let latest = latest_as_of(pool).await?;
    if latest.is_empty() {
        return Ok(DriftReport {
            total_value: 0.0, tradable_value: 0.0, owned_value: 0.0,
            active_mode: "crab".into(), abs_drift_pp: 0.0,
            categories: vec![],
        });
    }

    // Investment-account values per category (drives drift math)
    let cat_values = category_values(pool, &latest).await?;
    let tradable: f64 = cat_values.values().sum();

    // Owned-asset value (is_investment = 0) — counted in net worth but not in drift
    let owned: f64 = sqlx::query_scalar::<_, f64>(
        "SELECT COALESCE(SUM(s.value_usd) * 1.0, 0.0)
         FROM snapshots s
         JOIN accounts ac ON ac.id = s.account_id
         WHERE s.as_of = ?1 AND ac.is_investment = 0",
    ).bind(&latest).fetch_one(pool).await?;
    let total = tradable + owned;

    let active_mode = overall_market_mode(pool).await?;

    // Targets for the active market mode (investment categories only)
    let targets: Vec<(String, f64)> = sqlx::query_as(
        "SELECT category, target_pct FROM allocation_targets WHERE market_mode = ?1",
    ).bind(&active_mode).fetch_all(pool).await?;

    let mut categories: Vec<CategoryDrift> = targets.into_iter()
        .map(|(category, target_pct)| {
            let current_value = *cat_values.get(&category).unwrap_or(&0.0);
            let current_pct = if tradable > 0.0 { current_value / tradable } else { 0.0 };
            let drift_pp = (current_pct - target_pct) * 100.0;
            let target_value = target_pct * tradable;
            let adjustment_usd = target_value - current_value;
            let drift_usd = -adjustment_usd; // signed positive = over target
            CategoryDrift {
                category,
                current_value,
                current_pct,
                target_pct,
                drift_pp,
                drift_usd,
                adjustment_usd,
                tradable: true,
            }
        }).collect();
    categories.sort_by(|a, b| b.current_value.partial_cmp(&a.current_value).unwrap());

    let abs_drift_pp: f64 = categories.iter()
        .map(|c| c.drift_pp.abs())
        .sum::<f64>() / 2.0;

    Ok(DriftReport {
        total_value: total,
        tradable_value: tradable,
        owned_value: owned,
        active_mode,
        abs_drift_pp,
        categories,
    })
}

// ────────── Concentration ──────────

#[derive(Serialize)]
pub struct TopAsset { pub symbol: String, pub value_usd: f64, pub pct: f64 }

#[derive(Serialize)]
pub struct ConcentrationReport {
    pub total_value: f64,
    pub top1: Option<TopAsset>,
    pub top3_pct: f64,
    pub hhi: f64,           // 0..10000
    pub n_effective: f64,   // 1 / Σwᵢ²
}

pub async fn concentration(State(s): State<AppState>) -> Result<Json<ConcentrationReport>, AppError> {
    Ok(Json(compute_concentration(&s.pool).await?))
}

async fn compute_concentration(pool: &SqlitePool) -> Result<ConcentrationReport, AppError> {
    let latest = latest_as_of(pool).await?;
    if latest.is_empty() {
        return Ok(ConcentrationReport { total_value: 0.0, top1: None, top3_pct: 0.0, hhi: 0.0, n_effective: 0.0 });
    }
    // Aggregate per asset symbol across investment accounts. Owned-asset accounts and
    // the STOCKS_TOTAL synthetic are excluded — concentration only matters for tradable holdings.
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT a.symbol, SUM(s.value_usd) * 1.0
         FROM snapshots s
         JOIN assets a   ON a.id = s.asset_id
         JOIN accounts ac ON ac.id = s.account_id
         WHERE s.as_of = ?1
           AND ac.is_investment = 1
           AND a.symbol != 'STOCKS_TOTAL'
         GROUP BY a.symbol
         ORDER BY SUM(s.value_usd) DESC",
    ).bind(&latest).fetch_all(pool).await?;

    let total: f64 = rows.iter().map(|(_, v)| *v).sum();
    if total <= 0.0 {
        return Ok(ConcentrationReport { total_value: 0.0, top1: None, top3_pct: 0.0, hhi: 0.0, n_effective: 0.0 });
    }
    let weights: Vec<f64> = rows.iter().map(|(_, v)| v / total).collect();
    let hhi: f64 = weights.iter().map(|w| w * w).sum::<f64>() * 10_000.0;
    let sum_w2: f64 = weights.iter().map(|w| w * w).sum::<f64>();
    let n_effective = if sum_w2 > 0.0 { 1.0 / sum_w2 } else { 0.0 };
    let top1 = rows.first().map(|(symbol, value)| TopAsset {
        symbol: symbol.clone(), value_usd: *value, pct: value / total,
    });
    let top3_pct: f64 = rows.iter().take(3).map(|(_, v)| v / total).sum();

    Ok(ConcentrationReport { total_value: total, top1, top3_pct, hhi, n_effective })
}

// ────────── Net-worth deltas ──────────

#[derive(Serialize)]
pub struct DeltaPoint {
    pub label: String,
    pub days: i64,
    pub from_value: f64,
    pub to_value: f64,
    pub delta_usd: f64,
    pub delta_pct: f64,
}

#[derive(Serialize)]
pub struct DeltasReport {
    pub current: f64,
    pub current_as_of: String,
    pub points: Vec<DeltaPoint>,
    pub sparkline: Vec<(String, f64)>, // last ~30 daily NW points for hero
}

pub async fn networth_deltas(State(s): State<AppState>) -> Result<Json<DeltasReport>, AppError> {
    Ok(Json(compute_deltas(&s.pool).await?))
}

async fn compute_deltas(pool: &SqlitePool) -> Result<DeltasReport, AppError> {
    let latest = latest_as_of(pool).await?;
    if latest.is_empty() {
        return Ok(DeltasReport { current: 0.0, current_as_of: "".into(), points: vec![], sparkline: vec![] });
    }

    // All snapshot dates with totals — single round trip
    let series: Vec<(String, f64)> = sqlx::query_as(
        "SELECT as_of, SUM(value_usd) * 1.0 FROM snapshots GROUP BY as_of ORDER BY as_of ASC",
    ).fetch_all(pool).await?;

    let current = series.last().map(|(_, v)| *v).unwrap_or(0.0);
    let current_date = series.last().map(|(d, _)| d.clone()).unwrap_or_default();

    // Helper: find value at-or-before a given date.
    let value_at_or_before = |target: &str| -> Option<(String, f64)> {
        series.iter().rev()
            .find(|(d, _)| d.as_str() <= target)
            .cloned()
    };

    let parse = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
    let cur_date = parse(&current_date);

    let make = |label: &str, from_date: chrono::NaiveDate| -> Option<DeltaPoint> {
        let target = from_date.format("%Y-%m-%d").to_string();
        let (from_iso, from_val) = value_at_or_before(&target)?;
        let from_d = parse(&from_iso)?;
        let cd = cur_date?;
        let days = (cd - from_d).num_days();
        let delta = current - from_val;
        let pct = if from_val > 0.0 { delta / from_val * 100.0 } else { 0.0 };
        Some(DeltaPoint { label: label.into(), days, from_value: from_val, to_value: current, delta_usd: delta, delta_pct: pct })
    };

    let mut points = vec![];
    if let Some(cd) = cur_date {
        if let Some(p) = make("7d",   cd - chrono::Duration::days(7))   { points.push(p); }
        if let Some(p) = make("30d",  cd - chrono::Duration::days(30))  { points.push(p); }
        if let Some(p) = make("90d",  cd - chrono::Duration::days(90))  { points.push(p); }
        if let Some(p) = make("YTD",  chrono::NaiveDate::from_ymd_opt(cd.year_ce().1 as i32, 1, 1).unwrap_or(cd)) { points.push(p); }
    }

    // Sparkline: last ~30 most-recent points (or the whole series if shorter)
    let take = series.len().saturating_sub(30);
    let sparkline: Vec<(String, f64)> = series.iter().skip(take).cloned().collect();

    Ok(DeltasReport { current, current_as_of: current_date, points, sparkline })
}

// `chrono` types used above are imported via the helper alias — explicit use:
use chrono::Datelike;

// ────────── Actions ──────────

#[derive(Serialize, Clone)]
pub struct ActionCard {
    pub id: String,                 // stable key (e.g. "drift:crypto", "concentration:BTC")
    pub kind: String,               // "rebalance" | "concentration" | "cash_drag" | "snapshot" | "regime"
    pub priority: String,           // "critical" | "suggested" | "info"
    pub urgency: f64,               // 0..100
    pub headline: String,
    pub body: String,
    pub category: Option<String>,
    pub amount_usd: Option<f64>,
    pub direction: Option<String>,  // "buy" | "sell" | "snapshot"
}

#[derive(Serialize)]
pub struct ActionsReport {
    pub generated_at: String,
    pub active_mode: String,
    pub actions: Vec<ActionCard>,
    pub stale_data: bool,
    pub stale_days: i64,
}

pub async fn actions(State(s): State<AppState>) -> Result<Json<ActionsReport>, AppError> {
    Ok(Json(compute_actions(&s.pool).await?))
}

async fn compute_actions(pool: &SqlitePool) -> Result<ActionsReport, AppError> {
    let latest = latest_as_of(pool).await?;
    let now = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let stale_days = if latest.is_empty() { i64::MAX } else {
        let l = chrono::NaiveDate::parse_from_str(&latest, "%Y-%m-%d").ok();
        let n = chrono::NaiveDate::parse_from_str(&now, "%Y-%m-%d").ok();
        match (l, n) { (Some(l), Some(n)) => (n - l).num_days(), _ => 0 }
    };
    let stale = stale_days > STALE_SNAPSHOT_DAYS;
    let has_data = !latest.is_empty();

    let drift_report = compute_drift(pool).await?;
    let conc = compute_concentration(pool).await?;

    let mut out: Vec<ActionCard> = vec![];

    if !has_data {
        out.push(ActionCard {
            id: "snapshot:none".into(), kind: "snapshot".into(),
            priority: tier(URGENT_SCORE).into(), urgency: 100.0,
            headline: "No snapshots yet".into(),
            body: "Add your first snapshot in /data?tab=snapshots to start tracking. The Action Center fills in once data exists.".into(),
            category: None, amount_usd: None, direction: Some("snapshot".into()),
        });
    } else if stale {
        out.push(ActionCard {
            id: "snapshot:stale".into(), kind: "snapshot".into(),
            priority: tier(URGENT_SCORE).into(), urgency: 100.0,
            headline: format!("Update holdings — last snapshot {} days old", stale_days),
            body: "Your data is stale; rebalance signals below may be inaccurate. Click Snapshot in the nav.".into(),
            category: None, amount_usd: None, direction: Some("snapshot".into()),
        });
    }

    let total = drift_report.total_value;
    let min_trade = (total * MIN_TRADE_SIZE_FRAC).max(MIN_TRADE_SIZE_USD);

    // Drift-driven rebalance actions (one per tradable category breaching thresholds).
    for c in &drift_report.categories {
        if !c.tradable { continue; }
        if c.drift_pp.abs() < DRIFT_MIN_PP { continue; }
        if c.adjustment_usd.abs() < DRIFT_MIN_USD.min(min_trade) { continue; }
        if c.adjustment_usd.abs() < min_trade { continue; }

        let drift_score   = (c.drift_pp.abs() / 10.0).clamp(0.0, 1.0) * 40.0;
        let conc_score    = conc.top1.as_ref().map(|t| ((t.pct - 0.20) / 0.30).clamp(0.0, 1.0) * 20.0).unwrap_or(0.0);
        // Sentiment alignment: selling overweight in bear is good (1.0); buying overweight in bear bad (0.0); crab neutral (0.5)
        let alignment = sentiment_alignment(&c.category, c.adjustment_usd, &drift_report.active_mode);
        let sent_score = alignment * 25.0;
        let recency_score = 15.0; // no action_log yet — assume fresh

        let urgency = (drift_score + conc_score + sent_score + recency_score).clamp(0.0, 100.0);
        let priority = tier(urgency);
        if priority == "info" { continue; }

        let (verb, dir) = if c.adjustment_usd > 0.0 { ("Buy", "buy") } else { ("Sell", "sell") };
        let cat_label = humanize_cat(&c.category);
        let headline = format!("{verb} ~${} of {} to hit {} target",
            fmt_thousands(c.adjustment_usd.abs()), cat_label, drift_report.active_mode.to_uppercase());
        let body = format!("{} is {:.1}pp {} target ({:.1}% vs {:.0}%) under current {} mode.",
            cat_label, c.drift_pp.abs(),
            if c.drift_pp > 0.0 { "over" } else { "under" },
            c.current_pct * 100.0, c.target_pct * 100.0,
            drift_report.active_mode);

        out.push(ActionCard {
            id: format!("drift:{}", c.category),
            kind: "rebalance".into(),
            priority: priority.into(), urgency,
            headline, body,
            category: Some(c.category.clone()),
            amount_usd: Some(c.adjustment_usd.abs()),
            direction: Some(dir.into()),
        });
    }

    // Concentration risk
    if let Some(top) = &conc.top1 {
        if top.pct >= CONCENTRATION_TOP1_THRESHOLD {
            let urgency = (((top.pct - 0.20) / 0.30).clamp(0.0, 1.0) * 60.0 + 30.0).min(100.0);
            let trim = (top.pct - CONCENTRATION_TOP1_THRESHOLD) * conc.total_value;
            out.push(ActionCard {
                id: format!("concentration:{}", top.symbol),
                kind: "concentration".into(),
                priority: tier(urgency).into(), urgency,
                headline: format!("Trim {} — sits at {:.0}% of portfolio", top.symbol, top.pct * 100.0),
                body: format!("Single-asset risk above {:.0}% threshold. Sell ~${} to bring back to target.",
                    CONCENTRATION_TOP1_THRESHOLD * 100.0, fmt_thousands(trim)),
                category: None,
                amount_usd: Some(trim),
                direction: Some("sell".into()),
            });
        }
    }

    // Owned-asset stale-snapshot nudges (Car, future house, etc.)
    // Surface a low-priority "refresh value" action when the most recent snapshot for
    // an owned account is older than STALE_OWNED_DAYS (or when none exists).
    let owned_status: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT ac.id, ac.name, MAX(s.as_of)
         FROM accounts ac
         LEFT JOIN snapshots s ON s.account_id = ac.id
         WHERE ac.is_investment = 0 AND ac.active = 1
         GROUP BY ac.id, ac.name",
    ).fetch_all(pool).await?;
    let now_d = chrono::NaiveDate::parse_from_str(&now, "%Y-%m-%d").ok();
    for (account_id, name, last_snap) in owned_status {
        let days = match (&last_snap, now_d) {
            (Some(s), Some(n)) => chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .ok().map(|d| (n - d).num_days()).unwrap_or(i64::MAX),
            _ => i64::MAX, // never snapshotted
        };
        if days < STALE_OWNED_DAYS { continue; }
        // Urgency: 50 at 90d, 80 at 180d, 100 at 365d+ or never-snapshotted
        let urgency = if days == i64::MAX { 100.0 }
                      else { ((days as f64 / 365.0).min(1.0) * 50.0 + 50.0).min(100.0) };
        let label = if days == i64::MAX { "never set".to_string() } else { format!("{days} days ago") };
        out.push(ActionCard {
            id: format!("stale_owned:{account_id}"),
            kind: "refresh_value".into(),
            priority: tier(urgency).into(),
            urgency,
            headline: format!("Update {name} value — last set {label}"),
            body: "Owned assets need manual snapshots. Look up current market value (e.g. KBB for vehicles, Zillow for property) and add a snapshot in /data?tab=snapshots.".into(),
            category: None,
            amount_usd: None,
            direction: Some("snapshot".into()),
        });
    }

    // Sort highest urgency first
    out.sort_by(|a, b| b.urgency.partial_cmp(&a.urgency).unwrap());

    Ok(ActionsReport { generated_at: now, active_mode: drift_report.active_mode, actions: out, stale_data: stale, stale_days })
}

fn sentiment_alignment(category: &str, adjustment_usd: f64, mode: &str) -> f64 {
    // Selling = adjustment_usd < 0 (overweight). Buying = adjustment_usd > 0 (underweight).
    let selling = adjustment_usd < 0.0;
    let growth_categories = matches!(category, "stocks" | "crypto");
    let safety_categories = matches!(category, "cash" | "stable_yielding");
    match mode {
        "bull" => {
            if growth_categories && !selling { 1.0 }      // buy growth in bull = aligned
            else if safety_categories && selling { 1.0 }  // sell safety in bull = aligned
            else if growth_categories && selling { 0.5 }  // trimming overweight growth — defensible
            else { 0.5 }
        }
        "bear" => {
            if safety_categories && !selling { 1.0 }      // buy safety in bear = aligned
            else if growth_categories && selling { 1.0 }  // sell growth in bear = aligned
            else if safety_categories && selling { 0.0 }
            else { 0.5 }
        }
        _ => 0.5, // crab / unknown
    }
}

fn tier(score: f64) -> &'static str {
    if score >= URGENT_SCORE { "critical" }
    else if score >= SUGGESTED_SCORE { "suggested" }
    else { "info" }
}

fn humanize_cat(c: &str) -> String {
    match c {
        "stable_yielding" => "Stables".into(),
        "stocks" => "Stocks".into(),
        "crypto" => "Crypto".into(),
        "cash" => "Cash".into(),
        "car" => "Car".into(),
        other => other.to_string(),
    }
}

fn fmt_thousands(v: f64) -> String {
    let n = v.round() as i64;
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { out.insert(0, ','); }
        out.insert(0, ch);
    }
    if n < 0 { out.insert(0, '-'); }
    out
}

// ────────── Summary (single bundled call) ──────────

#[derive(Serialize)]
pub struct SummaryStats {
    pub net_worth: f64,
    pub as_of: String,
    pub abs_drift_pp: f64,
    pub deployment_ratio: f64,
    pub mode_coherence: ModeCoherence,
    pub active_mode: String,
}

#[derive(Serialize)]
pub struct ModeCoherence {
    pub configured: String,    // active mode in use
    pub stocks: String,        // signal from VOO
    pub crypto: String,        // signal from BTC
    pub aligned: bool,
}

#[derive(Serialize)]
pub struct SummaryReport {
    pub stats: SummaryStats,
    pub deltas: DeltasReport,
    pub drift: DriftReport,
    pub concentration: ConcentrationReport,
    pub actions: ActionsReport,
    pub sentiment_compact: Vec<SentimentBadge>,
}

#[derive(Serialize)]
pub struct SentimentBadge {
    pub category: String,
    pub signal: String,
    pub status: String,
}

pub async fn summary(State(s): State<AppState>) -> Result<Json<SummaryReport>, AppError> {
    let drift = compute_drift(&s.pool).await?;
    let concentration = compute_concentration(&s.pool).await?;
    let deltas = compute_deltas(&s.pool).await?;
    let actions = compute_actions(&s.pool).await?;

    // Deployment ratio: working capital as fraction of tradable
    let tradable_total = drift.tradable_value;
    let working: f64 = drift.categories.iter()
        .filter(|c| matches!(c.category.as_str(), "stocks" | "crypto" | "stable_yielding"))
        .map(|c| c.current_value).sum();
    let deployment_ratio = if tradable_total > 0.0 { working / tradable_total } else { 0.0 };

    // Mode coherence
    let signals = compact_signals(&s.pool).await?;
    let stocks_sig = signals.iter().find(|x| x.category == "stocks").map(|x| x.signal.clone()).unwrap_or_else(|| "pending".into());
    let crypto_sig = signals.iter().find(|x| x.category == "crypto").map(|x| x.signal.clone()).unwrap_or_else(|| "pending".into());
    let aligned = (stocks_sig == drift.active_mode || stocks_sig == "pending")
               && (crypto_sig == drift.active_mode || crypto_sig == "pending");

    let stats = SummaryStats {
        net_worth: deltas.current,
        as_of: deltas.current_as_of.clone(),
        abs_drift_pp: drift.abs_drift_pp,
        deployment_ratio,
        mode_coherence: ModeCoherence {
            configured: drift.active_mode.clone(),
            stocks: stocks_sig, crypto: crypto_sig, aligned,
        },
        active_mode: drift.active_mode.clone(),
    };

    Ok(Json(SummaryReport {
        stats, deltas, drift, concentration, actions,
        sentiment_compact: signals,
    }))
}

// ────────── Wealth (unified holdings) ──────────

#[derive(Serialize)]
pub struct Holding {
    pub symbol: String,
    pub name: Option<String>,
    pub type_code: String,
    pub category: String,
    pub account_name: String,
    pub quantity: f64,
    pub last_price: Option<f64>,
    pub value_usd: f64,
    pub pct_of_portfolio: f64,
    pub category_drift_pp: f64,
    pub recent_prices: Vec<f64>, // last ~30 daily closes
    pub price_change_pct_24h: Option<f64>,
}

#[derive(Serialize)]
pub struct WealthReport {
    pub as_of: String,
    pub total_value: f64,
    pub holdings: Vec<Holding>,
}

pub async fn wealth(State(s): State<AppState>) -> Result<Json<WealthReport>, AppError> {
    let latest = latest_as_of(&s.pool).await?;
    if latest.is_empty() {
        return Ok(Json(WealthReport { as_of: "".into(), total_value: 0.0, holdings: vec![] }));
    }
    // Per (asset, account) snapshot rows
    let rows: Vec<(i64, String, Option<String>, String, String, f64, Option<f64>, f64)> = sqlx::query_as(
        "SELECT a.id, a.symbol, a.name, a.type_code, ac.name, s.quantity * 1.0, s.price_usd, s.value_usd * 1.0
         FROM snapshots s
         JOIN assets a ON a.id = s.asset_id
         JOIN accounts ac ON ac.id = s.account_id
         WHERE s.as_of = ?1
           AND a.symbol != 'STOCKS_TOTAL'
         ORDER BY s.value_usd DESC",
    ).bind(&latest).fetch_all(&s.pool).await?;

    let total: f64 = rows.iter().map(|(_, _, _, _, _, _, _, v)| *v).sum();

    // Drift per category for the active mode (single computation reused per row)
    let drift_report = compute_drift(&s.pool).await?;
    let drift_by_cat: HashMap<String, f64> = drift_report.categories.iter()
        .map(|c| (c.category.clone(), c.drift_pp)).collect();

    let mut holdings: Vec<Holding> = Vec::with_capacity(rows.len());
    for (asset_id, symbol, name, type_code, acct_name, qty, price, value) in rows {
        let category = if acct_name == "Car" { "car".to_string() } else {
            match type_code.as_str() {
                "stock" => "stocks".into(),
                "stable" => "stable_yielding".into(),
                "crypto" | "nft" => "crypto".into(),
                "fiat" => "cash".into(),
                other => other.to_string(),
            }
        };
        let pct = if total > 0.0 { value / total } else { 0.0 };
        let drift_pp = *drift_by_cat.get(&category).unwrap_or(&0.0);

        // Recent prices (last 30 daily closes) for sparkline + 24h change
        let recent: Vec<(f64,)> = sqlx::query_as(
            "SELECT price_usd * 1.0 FROM price_history
             WHERE asset_id = ?1
             ORDER BY as_of DESC LIMIT 30",
        ).bind(asset_id).fetch_all(&s.pool).await?;
        let recent_prices: Vec<f64> = recent.into_iter().rev().map(|(p,)| p).collect();
        let change_24h = if recent_prices.len() >= 2 {
            let prev = recent_prices[recent_prices.len() - 2];
            let cur = recent_prices[recent_prices.len() - 1];
            if prev > 0.0 { Some((cur - prev) / prev * 100.0) } else { None }
        } else { None };

        holdings.push(Holding {
            symbol, name, type_code, category, account_name: acct_name,
            quantity: qty, last_price: price, value_usd: value,
            pct_of_portfolio: pct, category_drift_pp: drift_pp,
            recent_prices, price_change_pct_24h: change_24h,
        });
    }

    Ok(Json(WealthReport { as_of: latest, total_value: total, holdings }))
}

// ────────── Helpers ──────────

async fn latest_as_of(pool: &SqlitePool) -> Result<String, AppError> {
    let latest: String = sqlx::query_scalar("SELECT COALESCE(MAX(as_of), '') FROM snapshots")
        .fetch_one(pool).await?;
    Ok(latest)
}

/// Sum value_usd grouped by investment category. Owned accounts (is_investment=0) are
/// excluded — they contribute to net worth but never to drift/allocation math.
async fn category_values(pool: &SqlitePool, as_of: &str) -> Result<HashMap<String, f64>, AppError> {
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT a.type_code, SUM(s.value_usd) * 1.0
         FROM snapshots s
         JOIN assets a   ON a.id = s.asset_id
         JOIN accounts ac ON ac.id = s.account_id
         WHERE s.as_of = ?1 AND ac.is_investment = 1
         GROUP BY a.type_code",
    ).bind(as_of).fetch_all(pool).await?;
    let mut out: HashMap<String, f64> = HashMap::new();
    for (type_code, val) in rows {
        let category = match type_code.as_str() {
            "stock" => "stocks".to_string(),
            "stable" => "stable_yielding".to_string(),
            "crypto" | "nft" => "crypto".to_string(),
            "fiat" => "cash".to_string(),
            other => other.to_string(),
        };
        *out.entry(category).or_default() += val;
    }
    // Zero-fill the four investment categories so callers don't have to handle Option
    for cat in ["stocks", "stable_yielding", "crypto", "cash"] {
        out.entry(cat.into()).or_insert(0.0);
    }
    Ok(out)
}

/// Compute the current overall market mode using majority vote across active signals.
/// Falls back to "crab" when no representative asset has enough price history.
async fn overall_market_mode(pool: &SqlitePool) -> Result<String, AppError> {
    let signals = compact_signals(pool).await?;
    let valid: Vec<&str> = signals.iter()
        .filter(|s| s.status == "active" || s.status == "partial")
        .map(|s| s.signal.as_str()).collect();
    if valid.is_empty() { return Ok("crab".into()); }
    let bull = valid.iter().filter(|s| **s == "bull").count();
    let bear = valid.iter().filter(|s| **s == "bear").count();
    let half = valid.len() as f64 / 2.0;
    if bull as f64 > half { Ok("bull".into()) }
    else if bear as f64 > half { Ok("bear".into()) }
    else { Ok("crab".into()) }
}

/// Lightweight per-rep-asset signal for the compact sentiment strip + mode determination.
async fn compact_signals(pool: &SqlitePool) -> Result<Vec<SentimentBadge>, AppError> {
    let rep_assets: Vec<(&str, &str, &str)> = vec![
        ("stocks", "VOO", "stock"),
        ("crypto", "BTC", "crypto"),
    ];
    let mut out = vec![];
    for (cat, symbol, type_code) in rep_assets {
        let prices: Vec<(f64,)> = sqlx::query_as(
            "SELECT ph.price_usd * 1.0
             FROM price_history ph
             JOIN assets a ON a.id = ph.asset_id
             WHERE a.symbol = ?1 AND a.type_code = ?2
             ORDER BY ph.as_of ASC",
        ).bind(symbol).bind(type_code).fetch_all(pool).await?;

        let n = prices.len();
        if n < 50 {
            out.push(SentimentBadge { category: cat.into(), signal: "pending".into(), status: "insufficient_data".into() });
            continue;
        }
        let vals: Vec<f64> = prices.into_iter().map(|(p,)| p).collect();
        let current = *vals.last().unwrap();
        let ma50 = vals[vals.len()-50..].iter().sum::<f64>() / 50.0;
        let ma200 = if n >= 200 { vals[vals.len()-200..].iter().sum::<f64>() / 200.0 } else { vals.iter().sum::<f64>() / vals.len() as f64 };
        let signal = if current > ma200 && ma50 > ma200 { "bull" }
                     else if current < ma200 && ma50 < ma200 { "bear" }
                     else { "crab" };
        let status = if n >= 200 { "active" } else { "partial" };
        out.push(SentimentBadge { category: cat.into(), signal: signal.into(), status: status.into() });
    }
    Ok(out)
}
