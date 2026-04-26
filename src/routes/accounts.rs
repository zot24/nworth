use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, State};

use crate::{error::AppError, models::account::Account, AppState};

#[derive(Template)]
#[template(path = "accounts.html")]
struct AccountsTemplate {
    accounts: Vec<Account>,
}

pub async fn list(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let accounts = sqlx::query_as::<_, Account>(
        "SELECT id, name, type_code, institution, chain_code, active, notes
         FROM accounts
         WHERE active = 1
         ORDER BY type_code, name",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(AccountsTemplate { accounts })
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
        "SELECT id, name, type_code, institution, chain_code, active, notes
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
