//! Integration tests for the REST API (/api/v1/*).
//! Full CRUD regression tests for every entity.

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
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

async fn json_req(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let req = match method {
        "GET" => Request::get(path).body(Body::empty()).unwrap(),
        "POST" => Request::post(path)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.unwrap_or("{}").to_string()))
            .unwrap(),
        "PUT" => Request::put(path)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.unwrap_or("{}").to_string()))
            .unwrap(),
        "DELETE" => Request::delete(path).body(Body::empty()).unwrap(),
        _ => panic!("unsupported method"),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let val = if text.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text))
    };
    (status, val)
}

// ────────── Accounts ──────────

#[tokio::test]
async fn accounts_crud_lifecycle() {
    let state = test_state().await;
    let app = build_app(state);

    // List — empty
    let (status, val) = json_req(&app, "GET", "/api/v1/accounts", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(val.as_array().unwrap().len(), 0);

    // Create
    let (status, val) = json_req(
        &app, "POST", "/api/v1/accounts",
        Some(r#"{"name":"TestBank","type_code":"bank","institution":"Test Corp"}"#),
    ).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(val["name"], "TestBank");
    assert_eq!(val["type_code"], "bank");
    assert_eq!(val["institution"], "Test Corp");
    assert_eq!(val["active"], 1);
    let id = val["id"].as_i64().unwrap();

    // Get by ID
    let (status, val) = json_req(&app, "GET", &format!("/api/v1/accounts/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(val["name"], "TestBank");

    // Update
    let (status, val) = json_req(
        &app, "PUT", &format!("/api/v1/accounts/{id}"),
        Some(r#"{"name":"TestBank Updated","type_code":"exchange","notes":"modified"}"#),
    ).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(val["name"], "TestBank Updated");
    assert_eq!(val["type_code"], "exchange");

    // List — has 1
    let (_, val) = json_req(&app, "GET", "/api/v1/accounts", None).await;
    assert_eq!(val.as_array().unwrap().len(), 1);

    // Delete — no references exist, so this hard-deletes (purges)
    let (status, val) = json_req(&app, "DELETE", &format!("/api/v1/accounts/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(val["purged"], true);
    assert_eq!(val["deactivated"], false);

    // Verify it's gone from the list
    let (_, val) = json_req(&app, "GET", "/api/v1/accounts", None).await;
    assert_eq!(val.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn account_with_snapshots_is_soft_deleted() {
    let state = test_state().await;
    sqlx::query("INSERT INTO accounts(name, type_code, active) VALUES('HasData','exchange',1)")
        .execute(&state.pool).await.unwrap();
    sqlx::query("INSERT INTO assets(symbol, type_code, active) VALUES('BTC','crypto',1)")
        .execute(&state.pool).await.unwrap();
    sqlx::query("INSERT INTO snapshots(as_of, account_id, asset_id, quantity, value_usd) VALUES('2026-01-01',1,1,1.0,50000)")
        .execute(&state.pool).await.unwrap();
    let app = build_app(state);

    let (status, val) = json_req(&app, "DELETE", "/api/v1/accounts/1", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(val["purged"], false);
    assert_eq!(val["deactivated"], true);

    // Account row still exists, just inactive
    let (_, val) = json_req(&app, "GET", "/api/v1/accounts/1", None).await;
    assert_eq!(val["active"], 0);
}

// ────────── Assets ──────────

#[tokio::test]
async fn assets_crud_lifecycle() {
    let state = test_state().await;
    let app = build_app(state);

    // Create
    let (status, val) = json_req(
        &app, "POST", "/api/v1/assets",
        Some(r#"{"symbol":"BTC","type_code":"crypto","coingecko_id":"bitcoin","risk_code":"cat1_safe","target_pct":0.7}"#),
    ).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(val["symbol"], "BTC");
    assert_eq!(val["coingecko_id"], "bitcoin");
    assert_eq!(val["risk_code"], "cat1_safe");
    let id = val["id"].as_i64().unwrap();

    // Update
    let (status, val) = json_req(
        &app, "PUT", &format!("/api/v1/assets/{id}"),
        Some(r#"{"symbol":"BTC","type_code":"crypto","name":"Bitcoin","target_pct":0.8}"#),
    ).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(val["name"], "Bitcoin");

    // Delete — no references, hard-delete (purge)
    let (status, val) = json_req(&app, "DELETE", &format!("/api/v1/assets/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(val["purged"], true);

    let (_, val) = json_req(&app, "GET", "/api/v1/assets", None).await;
    assert_eq!(val.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn asset_with_positions_is_soft_deleted() {
    let state = test_state().await;
    sqlx::query("INSERT INTO accounts(name, type_code, active) VALUES('Test','broker',1)")
        .execute(&state.pool).await.unwrap();
    sqlx::query("INSERT INTO assets(symbol, type_code, active) VALUES('VOO','stock',1)")
        .execute(&state.pool).await.unwrap();
    sqlx::query("INSERT INTO positions(account_id, asset_id, quantity, value_usd, as_of) VALUES(1,1,10.0,5000,'2026-01-01')")
        .execute(&state.pool).await.unwrap();
    let app = build_app(state);

    let (status, val) = json_req(&app, "DELETE", "/api/v1/assets/1", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(val["purged"], false);
    assert_eq!(val["deactivated"], true);

    let (_, val) = json_req(&app, "GET", "/api/v1/assets/1", None).await;
    assert_eq!(val["active"], 0);
}

// ────────── Income ──────────

#[tokio::test]
async fn income_crud_lifecycle() {
    let state = test_state().await;
    let app = build_app(state);

    // Create
    let (status, val) = json_req(
        &app, "POST", "/api/v1/income",
        Some(r#"{"as_of":"2026-04-01","salary_usd":8000,"bonus_usd":500,"taxes_usd":2000,"company":"Acme"}"#),
    ).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(val["salary_usd"], 8000.0);
    assert_eq!(val["company"], "Acme");
    let id = val["id"].as_i64().unwrap();

    // Update
    let (status, val) = json_req(
        &app, "PUT", &format!("/api/v1/income/{id}"),
        Some(r#"{"as_of":"2026-04-01","salary_usd":9000,"company":"Acme Corp"}"#),
    ).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(val["salary_usd"], 9000.0);
    assert_eq!(val["company"], "Acme Corp");

    // List
    let (_, val) = json_req(&app, "GET", "/api/v1/income", None).await;
    assert_eq!(val.as_array().unwrap().len(), 1);

    // Delete
    let (status, _) = json_req(&app, "DELETE", &format!("/api/v1/income/{id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, val) = json_req(&app, "GET", "/api/v1/income", None).await;
    assert_eq!(val.as_array().unwrap().len(), 0);
}

// ────────── Expenses ──────────

#[tokio::test]
async fn expenses_crud_lifecycle() {
    let state = test_state().await;
    let app = build_app(state);

    let (status, val) = json_req(
        &app, "POST", "/api/v1/expenses",
        Some(r#"{"as_of":"2026-04-01","amount_usd":3500,"place":"NYC"}"#),
    ).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(val["amount_usd"], 3500.0);
    let id = val["id"].as_i64().unwrap();

    let (status, val) = json_req(
        &app, "PUT", &format!("/api/v1/expenses/{id}"),
        Some(r#"{"as_of":"2026-04-01","amount_usd":4000,"place":"SF"}"#),
    ).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(val["amount_usd"], 4000.0);
    assert_eq!(val["place"], "SF");

    let (status, _) = json_req(&app, "DELETE", &format!("/api/v1/expenses/{id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

// ────────── Allocation Targets ──────────

#[tokio::test]
async fn targets_crud_lifecycle() {
    let state = test_state().await;
    let app = build_app(state);

    let (status, val) = json_req(
        &app, "POST", "/api/v1/targets",
        Some(r#"{"category":"stocks","market_mode":"bull","target_pct":0.60,"notes":"bull stocks"}"#),
    ).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(val["category"], "stocks");
    assert_eq!(val["market_mode"], "bull");
    assert_eq!(val["target_pct"], 0.60);
    let id = val["id"].as_i64().unwrap();

    // Upsert same (category, market_mode) — should update
    let (status, val) = json_req(
        &app, "POST", "/api/v1/targets",
        Some(r#"{"category":"stocks","market_mode":"bull","target_pct":0.65}"#),
    ).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(val["target_pct"], 0.65);

    // Upsert should not duplicate — check stocks/bull row reflects update
    let (_, val) = json_req(&app, "GET", "/api/v1/targets", None).await;
    let targets = val.as_array().unwrap();
    let stocks_bull = targets.iter()
        .find(|t| t["category"] == "stocks" && t["market_mode"] == "bull")
        .unwrap();
    assert_eq!(stocks_bull["target_pct"], 0.65, "upsert should update existing (category, market_mode) row");

    let (status, _) = json_req(&app, "DELETE", &format!("/api/v1/targets/{id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

// ────────── Snapshots ──────────

#[tokio::test]
async fn snapshots_create_and_list() {
    let state = test_state().await;

    // Seed an account + asset
    sqlx::query("INSERT INTO accounts(name,type_code,active) VALUES('Test','broker',1)")
        .execute(&state.pool).await.unwrap();
    sqlx::query("INSERT INTO assets(symbol,type_code,active) VALUES('VOO','stock',1)")
        .execute(&state.pool).await.unwrap();

    let app = build_app(state);

    // Create
    let (status, val) = json_req(
        &app, "POST", "/api/v1/snapshots",
        Some(r#"{"as_of":"2026-04-01","account_id":1,"asset_id":1,"quantity":100,"value_usd":50000}"#),
    ).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(val["quantity"], 100.0);
    assert_eq!(val["value_usd"], 50000.0);
    let id = val["id"].as_i64().unwrap();

    // List (defaults to latest date)
    let (_, val) = json_req(&app, "GET", "/api/v1/snapshots", None).await;
    assert_eq!(val.as_array().unwrap().len(), 1);

    // List by date
    let (_, val) = json_req(&app, "GET", "/api/v1/snapshots?as_of=2026-04-01", None).await;
    assert_eq!(val.as_array().unwrap().len(), 1);

    // Upsert same (as_of, account, asset) — updates
    let (status, val) = json_req(
        &app, "POST", "/api/v1/snapshots",
        Some(r#"{"as_of":"2026-04-01","account_id":1,"asset_id":1,"quantity":110,"value_usd":55000}"#),
    ).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(val["quantity"], 110.0);

    // Still 1 row (upsert)
    let (_, val) = json_req(&app, "GET", "/api/v1/snapshots?as_of=2026-04-01", None).await;
    assert_eq!(val.as_array().unwrap().len(), 1);

    // Delete
    let (status, _) = json_req(&app, "DELETE", &format!("/api/v1/snapshots/{id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

// ────────── Positions ──────────

#[tokio::test]
async fn positions_upsert_and_delete() {
    let state = test_state().await;
    sqlx::query("INSERT INTO accounts(name,type_code,active) VALUES('Broker','broker',1)")
        .execute(&state.pool).await.unwrap();
    sqlx::query("INSERT INTO assets(symbol,type_code,active) VALUES('VOO','stock',1)")
        .execute(&state.pool).await.unwrap();

    let app = build_app(state);

    // Upsert
    let (status, val) = json_req(
        &app, "POST", "/api/v1/positions",
        Some(r#"{"account_id":1,"asset_id":1,"quantity":100,"avg_cost":500,"value_usd":50000}"#),
    ).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(val["quantity"], 100.0);

    // List
    let (_, val) = json_req(&app, "GET", "/api/v1/positions", None).await;
    assert_eq!(val.as_array().unwrap().len(), 1);

    // Delete
    let (status, _) = json_req(&app, "DELETE", "/api/v1/positions/1/1", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, val) = json_req(&app, "GET", "/api/v1/positions", None).await;
    assert_eq!(val.as_array().unwrap().len(), 0);
}

// ────────── Snapshot Trigger ──────────

#[tokio::test]
async fn snapshot_trigger_creates_from_positions() {
    let state = test_state().await;
    sqlx::query("INSERT INTO accounts(name,type_code,active) VALUES('Broker','broker',1)")
        .execute(&state.pool).await.unwrap();
    sqlx::query("INSERT INTO assets(symbol,type_code,active) VALUES('VOO','stock',1)")
        .execute(&state.pool).await.unwrap();
    sqlx::query("INSERT INTO positions(account_id,asset_id,quantity,avg_cost,last_price,value_usd,as_of) VALUES(1,1,100,500,510,51000,'2026-04-01')")
        .execute(&state.pool).await.unwrap();

    let app = build_app(state);

    let (status, val) = json_req(&app, "POST", "/api/v1/snapshots/trigger", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(val["snapshots_created"].as_u64().unwrap() >= 1);
}

// ────────── Chart APIs (read-only regression) ──────────

#[tokio::test]
async fn chart_apis_return_200_on_empty_db() {
    let state = test_state().await;
    let app = build_app(state);

    let endpoints = [
        "/api/networth",
        "/api/networth/by-category",
        "/api/allocation",
        "/api/allocation/stocks",
        "/api/allocation/crypto",
        "/api/allocation/adjustments",
        "/api/stables/apy",
        "/api/stocks/history",
        "/api/stocks/dividends",
        "/api/stocks/dividends/monthly",
        "/api/stocks/dividends/yearly",
        "/api/stocks/dividends/yoy",
        "/api/stocks/holdings",
        "/api/stocks/growth",
        "/api/stocks/normalized",
        "/api/crypto/history",
        "/api/cash/history",
        "/api/income/monthly",
        "/api/expenses/monthly",
        "/api/flow/monthly",
    ];

    for ep in endpoints {
        let (status, _) = json_req(&app, "GET", ep, None).await;
        assert_eq!(status, StatusCode::OK, "endpoint {ep} should return 200 on empty DB");
    }
}

// ────────── Page routes regression ──────────

#[tokio::test]
async fn all_page_routes_return_200() {
    let state = test_state().await;
    let app = build_app(state);

    let pages = [
        "/", "/stocks", "/crypto", "/cash", "/positions",
        "/income", "/expenses", "/flow",
        "/accounts", "/assets", "/targets", "/data",
        "/healthz",
    ];

    for page in pages {
        let req = Request::get(page).body(Body::empty()).unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "page {page} should return 200");
    }
}
