use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, State};

use crate::{error::AppError, models::account::Account, AppState};

/// One row per owned-type asset (Car, Apartment, Watch, …) — the "thing" itself,
/// joined to whichever owned-account container it lives in via its most recent
/// snapshot. This is what the Owned section of /accounts lists post-refactor:
/// the assets are the conceptual unit ("a car is a thing of value"), and the
/// container account is just the bucket they're grouped under.
pub struct OwnedAssetRow {
    pub asset_id: i64,
    pub asset_symbol: String,
    pub asset_name: String,
    pub account_id: Option<i64>,
    pub account_name: Option<String>,
    pub latest_value_usd: Option<f64>,
    pub latest_as_of: Option<String>,
}

#[derive(Template)]
#[template(path = "accounts.html")]
struct AccountsTemplate {
    investments: Vec<Account>,
    owned_assets: Vec<OwnedAssetRow>,
    total_account_count: usize,
}

pub async fn list(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let accounts = sqlx::query_as::<_, Account>(
        "SELECT id, name, type_code, institution, chain_code, active, notes, is_investment
         FROM accounts
         WHERE active = 1
         ORDER BY type_code, name",
    )
    .fetch_all(&state.pool)
    .await?;

    let total_account_count = accounts.len();
    let investments: Vec<Account> = accounts.into_iter()
        .filter(|a| a.is_investment == 1)
        .collect();

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

    Ok(AccountsTemplate { investments, owned_assets, total_account_count })
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
        "SELECT id, name, type_code, institution, chain_code, active, notes, is_investment
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
