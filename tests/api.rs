//! Integration tests for the HTTP surface.
//! Boot the full Axum app against an in-memory SQLite DB, seed a tiny portfolio,
//! and exercise every route.

use axum::{body::Body, http::{Request, StatusCode}};
use http_body_util::BodyExt;
use portfolio_tracker::{build_app, config::Config, migrate, AppState};
use sqlx::SqlitePool;
use tower::ServiceExt;

async fn test_state() -> AppState {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    migrate(&pool).await.unwrap();
    AppState {
        pool,
        cfg: Config {
            bind_addr: "127.0.0.1:0".into(),
            database_url: "sqlite::memory:".into(),
            coingecko_api_key: None,
            helius_rpc_url: None,
        },
    }
}

async fn seed(pool: &SqlitePool) {
    // 1 crypto asset, 1 stock asset
    sqlx::query(
        "INSERT INTO assets(symbol, type_code, coingecko_id, active)
         VALUES('BTC','crypto','bitcoin',1)",
    ).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO assets(symbol, type_code, active)
         VALUES('VOO','stock',1)",
    ).execute(pool).await.unwrap();

    // 1 crypto bucket account, 1 broker account
    sqlx::query(
        "INSERT INTO accounts(name, type_code, active)
         VALUES('Crypto','crypto',1)",
    ).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO accounts(name, type_code, active)
         VALUES('TestBroker','broker',1)",
    ).execute(pool).await.unwrap();

    for (as_of, btc_val, voo_val) in [
        ("2026-02-01", 100.0, 50.0),
        ("2026-03-01", 120.0, 55.0),
        ("2026-04-01", 150.0, 60.0),
    ] {
        sqlx::query(
            "INSERT INTO snapshots(as_of, account_id, asset_id, quantity, value_usd, source)
             SELECT ?, a.id, ast.id, 1.0, ?, 'test'
             FROM accounts a JOIN assets ast
             WHERE a.name = 'Crypto' AND ast.symbol = 'BTC'",
        )
        .bind(as_of).bind(btc_val).execute(pool).await.unwrap();
        sqlx::query(
            "INSERT INTO snapshots(as_of, account_id, asset_id, quantity, value_usd, source)
             SELECT ?, a.id, ast.id, 1.0, ?, 'test'
             FROM accounts a JOIN assets ast
             WHERE a.name = 'TestBroker' AND ast.symbol = 'VOO'",
        )
        .bind(as_of).bind(voo_val).execute(pool).await.unwrap();
    }
}

async fn get_body(app: axum::Router, path: &str) -> (StatusCode, String) {
    let resp = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn healthz_returns_ok() {
    let state = test_state().await;
    let app = build_app(state);
    let (status, body) = get_body(app, "/healthz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn dashboard_renders_with_empty_db() {
    let state = test_state().await;
    let app = build_app(state);
    let (status, body) = get_body(app, "/").await;
    assert_eq!(status, StatusCode::OK, "dashboard must render on empty DB");
    assert!(body.contains("No data yet"), "empty state should be shown");
}

#[tokio::test]
async fn dashboard_renders_action_center_with_data() {
    let state = test_state().await;
    seed(&state.pool).await;
    let app = build_app(state);
    let (status, body) = get_body(app, "/").await;
    assert_eq!(status, StatusCode::OK);
    // Action Center hydrates client-side via /api/insights/summary; just verify the shell rendered
    assert!(body.contains("hero-value"), "Action Center hero block should be present");
    assert!(body.contains("action-stack"), "Action card stack container should be present");
    assert!(body.contains("/api/insights/summary"), "page should fetch the summary endpoint");
}

#[tokio::test]
async fn insights_summary_returns_correct_totals() {
    let state = test_state().await;
    seed(&state.pool).await;
    let app = build_app(state);
    let (status, body) = get_body(app, "/api/insights/summary").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    // latest snapshot = 2026-04-01, total = 150 + 60 = 210
    assert_eq!(v["stats"]["net_worth"].as_f64().unwrap(), 210.0);
    assert_eq!(v["stats"]["as_of"], "2026-04-01");
    // active_mode falls back to crab when no price_history seeded
    assert_eq!(v["stats"]["active_mode"], "crab");
}

#[tokio::test]
async fn networth_api_returns_series() {
    let state = test_state().await;
    seed(&state.pool).await;
    let app = build_app(state);
    let (status, body) = get_body(app, "/api/networth").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let points = v["points"].as_array().expect("points should be an array");
    assert_eq!(points.len(), 3, "expected 3 monthly points");
    assert_eq!(points[2]["as_of"], "2026-04-01");
    assert_eq!(points[2]["value_usd"].as_f64().unwrap(), 210.0);

    let by_type = v["by_type"].as_array().unwrap();
    let types: Vec<&str> = by_type.iter().map(|t| t["type_code"].as_str().unwrap()).collect();
    assert!(types.contains(&"crypto"));
    assert!(types.contains(&"stock"));
}

#[tokio::test]
async fn allocation_api_returns_current_slices() {
    let state = test_state().await;
    seed(&state.pool).await;
    let app = build_app(state);
    let (status, body) = get_body(app, "/api/allocation").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let slices = v.as_array().unwrap();
    assert_eq!(slices.len(), 2);
    // biggest slice first (ORDER BY value_usd DESC)
    assert_eq!(slices[0]["symbol"], "BTC");
    assert_eq!(slices[0]["value_usd"].as_f64().unwrap(), 150.0);
    assert_eq!(slices[1]["symbol"], "VOO");
}

#[tokio::test]
async fn allocation_api_empty_db_returns_empty_array() {
    let state = test_state().await;
    let app = build_app(state);
    let (status, body) = get_body(app, "/api/allocation").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.trim(), "[]");
}

#[tokio::test]
async fn accounts_page_lists_accounts() {
    let state = test_state().await;
    seed(&state.pool).await;
    let app = build_app(state);
    let (status, body) = get_body(app, "/accounts").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Crypto"));
    assert!(body.contains("TestBroker"));
}

#[tokio::test]
async fn assets_page_lists_assets() {
    let state = test_state().await;
    seed(&state.pool).await;
    let app = build_app(state);
    let (status, body) = get_body(app, "/assets").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("BTC"));
    assert!(body.contains("VOO"));
}

#[tokio::test]
async fn unique_snapshot_constraint_works() {
    // schema-level guarantee: (as_of, account_id, asset_id) must be unique
    let state = test_state().await;
    seed(&state.pool).await;
    let dup = sqlx::query(
        "INSERT INTO snapshots(as_of, account_id, asset_id, quantity, value_usd, source)
         SELECT '2026-04-01', a.id, ast.id, 2.0, 999.0, 'test'
         FROM accounts a JOIN assets ast
         WHERE a.name = 'Crypto' AND ast.symbol = 'BTC'",
    )
    .execute(&state.pool)
    .await;
    assert!(dup.is_err(), "duplicate (as_of, account, asset) must be rejected");
}
