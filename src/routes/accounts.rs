use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, State};

use crate::{error::AppError, models::account::Account, AppState};

/// One row per owned-type asset (Car, Apartment, Watch, …) — the "thing" itself,
/// joined to whichever container account it lives in via its most recent
/// snapshot. This is what the Property section of /accounts lists.
pub struct OwnedAssetRow {
    pub asset_id: i64,
    pub asset_symbol: String,
    pub asset_name: String,
    pub account_id: Option<i64>,
    pub account_name: Option<String>,
    pub latest_value_usd: Option<f64>,
    pub latest_as_of: Option<String>,
}

/// One row per operating-role account, with its current cash balance summed
/// from the latest snapshot. Operating accounts hold fiat/cash-like assets,
/// not owned-type things, so we surface the rolled-up value rather than
/// itemizing per asset.
pub struct OperatingRow {
    pub account: Account,
    pub latest_value_usd: Option<f64>,
    pub latest_as_of: Option<String>,
}

#[derive(Template)]
#[template(path = "accounts.html")]
struct AccountsTemplate {
    investments: Vec<Account>,
    operating: Vec<OperatingRow>,
    owned_assets: Vec<OwnedAssetRow>,
    total_account_count: usize,
}

pub async fn list(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let accounts = sqlx::query_as::<_, Account>(
        "SELECT id, name, type_code, institution, chain_code, active, notes, role
         FROM accounts
         WHERE active = 1
         ORDER BY type_code, name",
    )
    .fetch_all(&state.pool)
    .await?;

    let total_account_count = accounts.len();
    let mut investments: Vec<Account> = Vec::new();
    let mut operating_accts: Vec<Account> = Vec::new();
    for a in accounts {
        match a.role.as_str() {
            "investment" => investments.push(a),
            "operating" => operating_accts.push(a),
            _ => { /* property containers surface via owned_assets below */ }
        }
    }

    // Operating: roll up the latest snapshot total per account so the section
    // can show "Citi: $X (as of …)" without itemizing every fiat row.
    let mut operating: Vec<OperatingRow> = Vec::with_capacity(operating_accts.len());
    for account in operating_accts {
        let latest: Option<(String, f64)> = sqlx::query_as(
            "WITH last_date AS (
                SELECT MAX(as_of) AS d FROM snapshots WHERE account_id = ?1
            )
            SELECT s.as_of, COALESCE(SUM(s.value_usd), 0.0)
              FROM snapshots s, last_date
             WHERE s.account_id = ?1 AND s.as_of = last_date.d
             GROUP BY s.as_of",
        )
        .bind(account.id)
        .fetch_optional(&state.pool)
        .await?;
        let (latest_as_of, latest_value_usd) = match latest {
            Some((d, v)) => (Some(d), Some(v)),
            None => (None, None),
        };
        operating.push(OperatingRow { account, latest_value_usd, latest_as_of });
    }

    // Owned assets joined to the account on their latest snapshot. A LEFT JOIN
    // keeps newly-added assets without snapshots visible (account_name = None).
    let owned_asset_rows: Vec<(i64, String, Option<String>, Option<i64>, Option<String>, Option<f64>, Option<String>)> = sqlx::query_as(
        "SELECT a.id, a.symbol, a.name,
                latest.account_id, ac.name,
                latest.value_usd, latest.as_of
         FROM assets a
         LEFT JOIN (
             SELECT s.asset_id,
                    s.account_id,
                    s.value_usd,
                    s.as_of,
                    ROW_NUMBER() OVER (PARTITION BY s.asset_id ORDER BY s.as_of DESC) AS rn
               FROM snapshots s
         ) latest ON latest.asset_id = a.id AND latest.rn = 1
         LEFT JOIN accounts ac ON ac.id = latest.account_id
         WHERE a.type_code = 'owned' AND a.active = 1
         ORDER BY a.symbol",
    )
    .fetch_all(&state.pool)
    .await?;

    let owned_assets: Vec<OwnedAssetRow> = owned_asset_rows.into_iter()
        .map(|(asset_id, asset_symbol, asset_name, account_id, account_name, latest_value_usd, latest_as_of)| OwnedAssetRow {
            asset_id, asset_symbol,
            asset_name: asset_name.unwrap_or_default(),
            account_id, account_name,
            latest_value_usd, latest_as_of,
        })
        .collect();

    Ok(AccountsTemplate { investments, operating, owned_assets, total_account_count })
}

#[derive(Debug)]
pub struct AccountHolding {
    pub id: i64,
    pub symbol: String,
    pub quantity: f64,
    pub value_usd: f64,
}

#[derive(Template)]
#[template(path = "account_detail.html")]
struct AccountDetailTemplate {
    account: Account,
    holdings: Vec<AccountHolding>,
    total_value: f64,
    latest_as_of: String,
}

pub async fn detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let account = sqlx::query_as::<_, Account>(
        "SELECT id, name, type_code, institution, chain_code, active, notes, role
         FROM accounts WHERE id = ?1",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    let latest_as_of: String = sqlx::query_scalar(
        "SELECT COALESCE(MAX(as_of), '') FROM snapshots WHERE account_id = ?1",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    let holdings: Vec<AccountHolding> = if latest_as_of.is_empty() {
        vec![]
    } else {
        let rows: Vec<(i64, String, f64, f64)> = sqlx::query_as(
            "SELECT s.id, a.symbol, s.quantity, s.value_usd
             FROM snapshots s
             JOIN assets a ON a.id = s.asset_id
             WHERE s.account_id = ?1 AND s.as_of = ?2
             ORDER BY s.value_usd DESC",
        )
        .bind(id)
        .bind(&latest_as_of)
        .fetch_all(&state.pool)
        .await?;

        rows.into_iter()
            .map(|(id, symbol, quantity, value_usd)| AccountHolding {
                id,
                symbol,
                quantity,
                value_usd,
            })
            .collect()
    };

    let total_value: f64 = holdings.iter().map(|h| h.value_usd).sum();

    Ok(AccountDetailTemplate {
        account,
        holdings,
        total_value,
        latest_as_of,
    })
}
