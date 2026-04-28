//! Library surface so integration tests can spin up the app.

use axum::{routing::{get, post, put, delete}, Router};
use sqlx::SqlitePool;
use tower_http::{compression::CompressionLayer, services::ServeDir, trace::TraceLayer};

pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod routes;
pub mod services;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub cfg: config::Config,
}

/// Build the Axum router wired to `state`. Used by `main.rs` and integration tests.
pub fn build_app(state: AppState) -> Router {
    Router::new()
        // Dashboard
        .route("/", get(routes::dashboard::index))
        // Holdings
        .route("/stocks", get(routes::stocks::index))
        .route("/crypto", get(routes::crypto::index))
        .route("/cash", get(routes::cash::index))
        .route("/positions", get(routes::positions::index))
        .route("/wealth", get(routes::wealth::index))
        // Financial
        .route("/income", get(routes::income::index))
        .route("/expenses", get(routes::expenses::index))
        .route("/flow", get(routes::flow::index))
        // Manage
        .route("/accounts", get(routes::accounts::list))
        .route("/accounts/:id", get(routes::accounts::detail))
        .route("/assets", get(routes::assets::list))
        .route("/targets", get(routes::targets::index))
        .route("/data", get(routes::data::index))


        // JSON APIs
        .route("/api/networth", get(routes::api::networth_series))
        .route("/api/allocation", get(routes::api::allocation))
        .route("/api/stocks/history", get(routes::api::stocks_history))
        .route("/api/stocks/dividends", get(routes::api::stocks_dividends))
        .route("/api/crypto/history", get(routes::api::crypto_history))
        .route("/api/cash/history", get(routes::api::cash_history))
        .route(
            "/api/accounts/:id/history",
            get(routes::api::account_history),
        )
        .route("/api/income/monthly", get(routes::api::income_monthly))
        .route("/api/expenses/monthly", get(routes::api::expenses_monthly))
        .route("/api/expenses/by-category", get(routes::api::expenses_by_category))
        .route("/api/flow/monthly", get(routes::api::flow_monthly))
        // New chart APIs
        .route("/api/networth/by-category", get(routes::api::networth_by_category))
        .route("/api/allocation/stocks", get(routes::api::allocation_stocks))
        .route("/api/allocation/crypto", get(routes::api::allocation_crypto))
        .route("/api/stocks/dividends/monthly", get(routes::api::dividends_monthly))
        .route("/api/stocks/dividends/yearly", get(routes::api::dividends_yearly))
        .route("/api/stocks/dividends/yoy", get(routes::api::dividends_yoy))
        .route("/api/stocks/holdings", get(routes::api::stocks_holdings))
        .route("/api/stocks/growth", get(routes::api::stocks_growth))
        .route("/api/stocks/normalized", get(routes::api::stocks_normalized))
        .route("/api/allocation/adjustments", get(routes::api::allocation_adjustments))
        .route("/api/stables/apy", get(routes::api::stables_apy))
        .route("/api/market/sentiment", get(routes::api::market_sentiment))
        // Insights — Action Center analytics
        .route("/api/insights/summary", get(routes::insights::summary))
        .route("/api/insights/drift", get(routes::insights::drift))
        .route("/api/insights/concentration", get(routes::insights::concentration))
        .route("/api/insights/networth/deltas", get(routes::insights::networth_deltas))
        .route("/api/insights/actions", get(routes::insights::actions))
        .route("/api/insights/wealth", get(routes::insights::wealth))
        // CRUD
        .route("/accounts/new", post(routes::crud::create_account))
        .route("/accounts/:id/edit", post(routes::crud::update_account))
        .route("/accounts/:id/delete", post(routes::crud::delete_account))
        .route("/assets/new", post(routes::crud::create_asset))
        .route("/assets/:id/edit", post(routes::crud::update_asset))
        .route("/assets/:id/delete", post(routes::crud::delete_asset))
        .route("/income/new", post(routes::crud::create_income))
        .route("/income/:id/delete", post(routes::crud::delete_income))
        .route("/expenses/new", post(routes::crud::create_expense))
        .route("/expenses/:id/delete", post(routes::crud::delete_expense))
        .route("/snapshots/new", post(routes::crud::create_snapshot))
        .route("/snapshots/:id/delete", post(routes::crud::delete_snapshot))
        .route("/snapshots/trigger", post(routes::crud::trigger_snapshots))
        .route("/targets/new", post(routes::crud::create_target))
        .route("/targets/:id/delete", post(routes::crud::delete_target))
        .route("/categories/new", post(routes::crud::create_category))
        .route("/categories/:id/delete", post(routes::crud::delete_category))
        .route("/labels/new", post(routes::crud::create_label))
        .route("/labels/:id/delete", post(routes::crud::delete_label))
        .route("/income/:id/edit", post(routes::crud::update_income))
        .route("/expenses/:id/edit", post(routes::crud::update_expense))
        .route("/positions/upsert", post(routes::crud::upsert_position_form))
        .route("/positions/track", post(routes::crud::track_holding))
        .route("/positions/:acct/:asset/delete", post(routes::crud::delete_position_form))
        .route("/snapshots/:id/edit", post(routes::crud::update_snapshot))
        // REST API v1 — full JSON CRUD
        .route("/api/v1/accounts", get(routes::rest::list_accounts).post(routes::rest::create_account_json))
        .route("/api/v1/accounts/:id", get(routes::rest::get_account).put(routes::rest::update_account_json).delete(routes::rest::delete_account_json))
        .route("/api/v1/assets", get(routes::rest::list_assets).post(routes::rest::create_asset_json))
        .route("/api/v1/assets/:id", get(routes::rest::get_asset).put(routes::rest::update_asset_json).delete(routes::rest::delete_asset_json))
        .route("/api/v1/snapshots", get(routes::rest::list_snapshots).post(routes::rest::create_snapshot_json))
        .route("/api/v1/snapshots/:id", delete(routes::rest::delete_snapshot_json))
        .route("/api/v1/snapshots/trigger", post(routes::rest::trigger_snapshot_json))
        .route("/api/v1/positions", get(routes::rest::list_positions).post(routes::rest::upsert_position))
        .route("/api/v1/positions/:acct/:asset", delete(routes::rest::delete_position))
        .route("/api/v1/income", get(routes::rest::list_income).post(routes::rest::create_income_json))
        .route("/api/v1/income/:id", put(routes::rest::update_income_json).delete(routes::rest::delete_income_json))
        .route("/api/v1/expenses", get(routes::rest::list_expenses).post(routes::rest::create_expense_json))
        .route("/api/v1/expenses/:id", put(routes::rest::update_expense_json).delete(routes::rest::delete_expense_json))
        .route("/api/v1/targets", get(routes::rest::list_targets).post(routes::rest::create_target_json))
        .route("/api/v1/targets/:id", delete(routes::rest::delete_target_json))
        .route("/api/v1/categories", get(routes::rest::list_categories).post(routes::rest::create_category_json))
        .route("/api/v1/categories/:id", put(routes::rest::update_category_json).delete(routes::rest::delete_category_json))
        .route("/api/v1/labels", get(routes::rest::list_labels).post(routes::rest::create_label_json))
        .route("/api/v1/labels/:id", put(routes::rest::update_label_json).delete(routes::rest::delete_label_json))
        // Docs
        .route("/guide", get(routes::guide::index))
        .route("/api/docs", get(routes::docs::swagger_ui))
        .route("/api/docs/openapi.yaml", get(routes::docs::openapi_spec))
        // Infra
        .route("/healthz", get(|| async { "ok" }))
        .nest_service("/static", ServeDir::new("static"))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Run migrations against the given pool. Call before building the app.
pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
