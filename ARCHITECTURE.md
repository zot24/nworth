# nworth — Architecture & Data Model

## Goal

Self-hosted personal portfolio tracker. Track net worth across stocks, crypto,
cash/stables, income, and expenses, with market-sentiment-aware allocation
targets and a decision-driving Action Center on the homepage.

Three binaries sharing one SQLite file:
- **Web app** (`nworth-web`) — serves pages, JSON APIs, REST API, Swagger docs
- **Pricefeed** (`nworth-feed`) — microservice that fetches prices from external APIs, caches in SQLite, creates daily snapshots
- **CLI** (`nworth-cli`) — command-line client for the REST API

---

## System Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                          User                                        │
│                                                                     │
│   Browser:     http://localhost:8080        (pages + charts)        │
│   API client:  http://localhost:8080/api/v1 (REST JSON CRUD)       │
│   API docs:    http://localhost:8080/api/docs (Swagger UI)         │
│   CLI:         nworth-cli accounts list                                 │
│                nworth-cli networth                                       │
│                nworth-cli expenses create '{"as_of":"…","amount_usd":500}'│
└───────────────────────────┬─────────────────────────────────────────┘
                            │ HTTP
                            ▼
┌─────────────────────────────────────────────────────────────────────┐
│               Web Server (nworth-web)                                │
│               cargo run --bin nworth-web                             │
│               http://localhost:8080                                   │
│                                                                     │
│   Pages (Askama HTML):                                              │
│     / /stocks /crypto /cash /positions /income /expenses            │
│     /flow /accounts /accounts/:id /assets /targets /data            │
│                                                                     │
│   Chart APIs (JSON, 20 endpoints):                                  │
│     /api/networth, /api/networth/by-category                       │
│     /api/allocation, /api/allocation/stocks, /api/allocation/crypto │
│     /api/allocation/adjustments                                     │
│     /api/stocks/history, /api/stocks/dividends/*                   │
│     /api/stocks/holdings, /api/stocks/growth, /api/stocks/normalized│
│     /api/crypto/history, /api/cash/history                         │
│     /api/accounts/:id/history                                       │
│     /api/income/monthly, /api/expenses/monthly, /api/flow/monthly  │
│     /api/stables/apy                                                │
│                                                                     │
│   REST API v1 (JSON CRUD, 7 entities):                              │
│     /api/v1/accounts   GET POST  | /api/v1/accounts/:id GET PUT DEL│
│     /api/v1/assets     GET POST  | /api/v1/assets/:id   GET PUT DEL│
│     /api/v1/snapshots  GET POST  | /api/v1/snapshots/:id     DEL   │
│     /api/v1/positions  GET POST  | /api/v1/positions/:a/:b   DEL   │
│     /api/v1/income     GET POST  | /api/v1/income/:id    PUT DEL   │
│     /api/v1/expenses   GET POST  | /api/v1/expenses/:id  PUT DEL   │
│     /api/v1/targets    GET POST  | /api/v1/targets/:id       DEL   │
│     /api/v1/snapshots/trigger POST                                  │
│                                                                     │
│   Docs:                                                             │
│     /api/docs           → Swagger UI                                │
│     /api/docs/openapi.yaml → OpenAPI 3.0 spec                      │
│                                                                     │
│   HTML CRUD (form POST → redirect):                                 │
│     /accounts/new, /assets/new, /income/new, /expenses/new ...     │
│                                                                     │
│   *** WEB APP READS FROM SQLITE ONLY — NO EXTERNAL API CALLS ***   │
└───────────────────────────┬─────────────────────────────────────────┘
                            │ SQLite
                            ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       data/portfolio.db                               │
│                                                                     │
│   Core:     accounts, assets, snapshots, positions                  │
│   Finance:  income, expenses, transactions (dividends)              │
│   Prices:   price_history, fx_rates                                 │
│   Config:   allocation_targets, loans, loan_allocations             │
│   Ref:      account_types, asset_types, risk_categories, chains     │
└───────────────────────────▲─────────────────────────────────────────┘
                            │ SQLite (write)
┌───────────────────────────┴─────────────────────────────────────────┐
│                  Pricefeed Microservice (nworth-feed)                 │
│                  cargo run --bin nworth-feed -- --loop 3600           │
│                                                                     │
│   1. Fetch crypto prices ──→ CoinGecko API ──→ price_history       │
│   2. Fetch stock prices  ──→ Yahoo Finance ──→ price_history       │
│   3. Fetch FX rates      ──→ exchangerate.host ──→ fx_rates        │
│   4. Update positions    ──→ positions.last_price, value_usd       │
│   5. Create daily snapshot ──→ snapshots (from positions × prices)  │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                  CLI Client (nworth-cli)                       │
│                  cargo run --bin nworth-cli -- <command>       │
│                                                                     │
│   Talks to the web server's REST API over HTTP.                     │
│   Supports: list, get, create, update, delete for all entities.     │
│   Also: networth, adjustments, apy                                  │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Data Import (one-time migration)

The xlsx import runs **once** to seed the database with historical data.
After migration, the app is the source of truth — new data via CRUD/API/pricefeed.

```
mine.xlsx (legacy spreadsheet — ONE-TIME IMPORT)
    │
    ▼
scripts/import_xlsx.py --db data/portfolio.db
    │
    ├── Cash - Data (Hidden)     → accounts + assets + snapshots
    ├── Stock - Data (Hidden)    → accounts + assets + snapshots + positions + dividends
    ├── Crypto - Data            → accounts + assets + snapshots
    ├── Expenses - Data (Hidden) → expenses
    ├── Income - Data (Hidden)   → income
    ├── The Loan (Hidden)        → loans + loan_allocations
    ├── Default Data (Hidden)    → fx_rates
    ├── Overall - Pivot Table    → allocation_targets
    └── Cash - Pivot Table       → snapshots (car value)
```

---

## Snapshot Lifecycle

Snapshots are the **core data** — every chart and net worth calculation reads from them.

```
How snapshots are created:

1. XLSX IMPORT (historical, one-time)
   Monthly snapshots from Oct 2019 – Apr 2026.

2. PRICEFEED (automatic, daily)
   After fetching prices, creates snapshot rows:
   positions × latest prices → snapshot with source='pricefeed'

3. MANUAL TRIGGER (on demand)
   Click "snapshot" in the nav bar or POST /api/v1/snapshots/trigger
   Creates snapshots from positions with source='manual'

4. REST API (programmatic)
   POST /api/v1/snapshots with explicit values.

All methods use ON CONFLICT upsert — UNIQUE(as_of, account_id, asset_id).
```

---

## Database Entity Diagram

```
┌─────────────────────┐          ┌─────────────────────┐
│    account_types     │          │     asset_types      │
├─────────────────────┤          ├─────────────────────┤
│ code  PK  TEXT      │          │ code  PK  TEXT      │
│ label     TEXT      │          │ label     TEXT      │
│                     │          │                     │
│ broker, bank, cash, │          │ crypto, stock,      │
│ exchange, crypto,   │          │ stable, fiat, nft   │
│ asset               │          │                     │
└────────┬────────────┘          └────────┬────────────┘
         │ FK                             │ FK
         ▼                                ▼
┌────────────────────────────────────────────────────────┐
│                      accounts                           │
├────────────────────────────────────────────────────────┤
│ id, name (UNIQUE), type_code (FK), institution,        │
│ chain_code (FK), active, notes, created_at             │
│ CRUD: /api/v1/accounts + /accounts page + /data page   │
└────────────────────┬───────────────────────────────────┘
                     │ 1:many
                     ▼
┌────────────────────────────────────────────────────────┐
│                      snapshots                          │
│              (THE CORE TABLE — drives all charts)       │
├────────────────────────────────────────────────────────┤
│ id, as_of, account_id (FK), asset_id (FK),             │
│ quantity, price_usd, value_usd, source                 │
│ UNIQUE(as_of, account_id, asset_id)                    │
│                                                        │
│ "On date X, account Y held Z of asset W, worth $N"    │
│ Created by: import, pricefeed, manual trigger, or API  │
└────────────────────────────────────────────────────────┘
                     ▲
┌────────────────────┴───────────────────────────────────┐
│                       assets                            │
├────────────────────────────────────────────────────────┤
│ id, symbol (UNIQUE w/ type_code), name, type_code (FK),│
│ chain_code, risk_code, coingecko_id, yahoo_ticker,     │
│ last_price, active                                     │
│ (last_price is updated by the pricefeed; positions     │
│ read it via JOIN — it's not stored on positions.)      │
│ CRUD: /api/v1/assets + /assets page + /data page       │
└────────────────────────────────────────────────────────┘

┌──────────────────────────┐  ┌──────────────────────┐
│        positions         │  │    transactions      │
│   (current holdings)     │  │  (dividends, etc.)   │
├──────────────────────────┤  ├──────────────────────┤
│ PK(account_id, asset_id) │  │ id, ts, account_id,  │
│ quantity, avg_cost,      │  │ asset_id, kind,      │
│ apy_pct, as_of           │  │ quantity, price_usd, │
│                          │  │ fee_usd, notes       │
│ apy_pct is per (account, │  │ 103 dividend rows    │
│ asset) so the same asset │  └──────────────────────┘
│ can yield at different   │
│ rates in different accts │
│ (Wise USD vs Chase USD). │
│ value_usd / last_price   │
│ are derived via JOIN on  │
│ assets at read time.     │
└──────────────────────────┘

┌──────────────┐ ┌──────────────┐ ┌──────────────────────┐
│    income    │ │   expenses   │ │  allocation_targets  │
├──────────────┤ ├──────────────┤ ├──────────────────────┤
│ id, as_of,   │ │ id, as_of,   │ │ id, category,        │
│ salary_usd,  │ │ amount_usd,  │ │ market_mode,         │
│ bonus_usd,   │ │ place, notes │ │ target_pct, notes    │
│ taxes_usd,   │ │              │ │                      │
│ company      │ │ 73 rows      │ │ UNIQUE(category,     │
│ 74 rows      │ └──────────────┘ │        market_mode)  │
└──────────────┘                   │ Σ per market_mode is │
                                   │ enforced ≤ 100% on   │
                                   │ /targets/new.        │
┌──────────────┐ ┌──────────────┐ │ Categories: stocks / │
│price_history │ │   fx_rates   │ │ stable_yielding /    │
├──────────────┤ ├──────────────┤ │ crypto / cash.       │
│ PK(asset_id, │ │ PK(ccy,      │ └──────────────────────┘
│    as_of)    │ │    as_of)    │
│ price_usd,   │ │ rate_usd,    │ ┌──────────────────────┐
│ source       │ │ source       │ │      settings        │
│ By pricefeed │ │ 3 rows       │ ├──────────────────────┤
└──────────────┘ └──────────────┘ │ key (PK), value,     │
                                   │ updated_at           │
                                   │ App-level prefs      │
                                   │ (display_currency).  │
                                   └──────────────────────┘
```

---

## Net Worth Calculation

```
Net Worth = SUM(snapshots.value_usd) WHERE as_of = latest date

Category mapping (/api/networth/by-category):
  type_code='stock'       → "Stocks"
  type_code='stable'      → "Stable Yielding"
  type_code='crypto'/'nft'→ "Crypto"
  type_code='fiat'        → "Cash"
  account.name='Car'      → "Car"
```

---

## Pivot Table Logic (replicated from xlsx)

```
Computation                           API endpoint                    Status
────────────────────────────────────────────────────────────────────────────
Current value per category            /api/networth/by-category       ✓
Desired % per category                /api/v1/targets                 ✓
Adjustment $ to hit targets           /api/allocation/adjustments     ✓
Stable Yielding APY (legacy, fixed 6%) /api/stables/apy                ✓
Passive income (per-position APY)     /api/yield/summary              ✓
Monthly/quarterly/yearly dividends    /api/stocks/dividends/*         ✓
YoY dividend growth                   /api/stocks/dividends/yearly    ✓
Per-holding details + P&L             /api/stocks/holdings            ✓
Cross-broker stock normalization      /api/stocks/normalized          ✓
Yearly portfolio growth               /api/stocks/growth              ✓
Risk category breakdown               server-side in crypto.rs        ✓
Actual vs target allocation           template JS                     ✓
```

---

## Testing

22 integration tests across `tests/api.rs` (10) and `tests/rest_api.rs` (12).

`tests/api.rs` covers: `/healthz`, page renders on empty + seeded DB, `/api/networth`,
`/api/allocation`, accounts/assets pages list from DB, unique-constraint on
`(as_of, account_id, asset_id)`, `/api/insights/summary` totals on seeded data,
all chart APIs returning 200 on an empty DB.

`tests/rest_api.rs` covers full CRUD lifecycles for accounts, assets, snapshots,
positions, income, expenses, targets — plus the smart-delete fallback path
(soft-delete when references exist, hard-delete when not) for accounts and assets,
and the snapshot-trigger from positions endpoint.

```
cargo test --release          # all 22
cargo test --release --test rest_api
cargo test --release --test api
```

---

## File Structure

```
.
├── Cargo.toml                ← 3 [[bin]] entries (nworth-web, nworth-feed, nworth-cli)
├── ARCHITECTURE.md
├── README.md
├── openapi.yaml              ← OpenAPI 3.0 spec, served at /api/docs
├── data/                     ← gitignored; SQLite lives here at runtime
├── migrations/               ← sqlx migrations, run on startup
├── src/
│   ├── main.rs               ← nworth-web entry
│   ├── lib.rs                ← Axum router + AppState
│   ├── config.rs, db.rs, error.rs
│   ├── bin/
│   │   ├── pricefeed.rs      ← nworth-feed entry
│   │   └── cli.rs            ← nworth-cli entry
│   ├── models/
│   ├── routes/               ← page handlers, JSON chart APIs, REST v1, insights, CRUD
│   └── services/             ← prices (CoinGecko + Yahoo), fx, solana (Helius)
├── templates/                ← Askama HTML; all extend base.html
├── tests/                    ← integration tests
├── Dockerfile
└── docker-compose.yml
```
