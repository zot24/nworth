# nworth

Self-hosted personal portfolio tracker. Net worth across stocks, crypto, stables, cash, and physical assets — with market-sentiment-aware allocation targets, decision-driving Action Center, expense categorization, and a clean REST API behind everything.

Rust (Axum) + SQLite + Askama. Three small binaries sharing one SQLite file. No Node build step, no auth layer, no cloud.

## Stack

| Layer | Choice | Why |
|---|---|---|
| Backend | Rust + Axum 0.7 | Fast, single binary, async without ceremony |
| DB | SQLite via sqlx 0.8 | Zero infra; WAL handles plenty of write volume |
| Templates | Askama + HTMX | Server-rendered, no Node toolchain |
| Charts | Chart.js (CDN) | Lightweight; matches the data-density we need |
| Styling | Custom CSS in `templates/base.html` | Strict design tokens; no framework |
| Deploy | Docker Compose | One command, volume-mounted DB |

## Binaries

| Binary | Role |
|---|---|
| `nworth-web` | Web server: pages, JSON APIs, REST API at `/api/v1`, Swagger at `/api/docs` |
| `nworth-feed` | Background fetcher for market prices + FX rates → writes to the same SQLite |
| `nworth-cli` | Command-line client for the REST API |

The cargo package is still named `portfolio-tracker` (the historical name); only the binaries carry the `nworth-*` namespace.

## Quick start

```bash
# 1. Install Rust if needed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Clone + cd

# 3. First run (creates ./data/portfolio.db, runs migrations)
cp .env.example .env
cargo run --release --bin nworth-web

# 4. Open http://localhost:8080
#    The homepage will show "No data yet" — click through to /data
#    to add your first account, asset, and snapshot.
```

Or via Docker:

```bash
docker compose up --build
```

The pricefeed is optional and runs as a separate process when you want it:

```bash
cargo run --release --bin nworth-feed -- --once          # one fetch
cargo run --release --bin nworth-feed -- --loop 3600     # every hour
```

## Pages

| Path | What |
|---|---|
| `/` | **Action Center** — net-worth hero with delta + sparkline; three decision stats (drift, deployed %, market mode); ranked action cards (rebalance, trim concentration, stale-snapshot warning); compact sentiment strip |
| `/wealth` | Single sortable, filterable table of every position across stocks/crypto/stables/cash/physical, with per-row drift bars and 30-day sparklines |
| `/stocks`, `/crypto`, `/cash`, `/positions`, `/income`, `/expenses`, `/flow`, `/accounts`, `/assets`, `/targets` | Read-only visualizations |
| `/data` | The single edit hub — tabs for accounts, assets, snapshots, positions, income, expenses, categories, labels, targets |
| `/guide` | In-app guide |
| `/api/docs` | Swagger UI for the REST API |

The app makes a hard separation: **`/data` is the only place anything mutates**. Every other page is read-only and links back via a small "edit in /data →" affordance. This keeps decision-driving views clean.

## Data model

Long-format throughout — adding a new asset is a row, never a schema change.

```
accounts (id, name, type_code, institution, chain_code, active, notes, role)
                                         ← role: investment | operating | property
assets (id, symbol, name, type_code, chain_code, risk_code,
        coingecko_id, yahoo_ticker, last_price, active)
snapshots (id, as_of, account_id, asset_id, quantity, price_usd,
           value_usd, source)            ← UNIQUE(as_of, account, asset)
positions (account_id, asset_id, quantity, avg_cost, apy_pct, as_of)
                                         ← current state. last_price + value_usd
                                            are derived from a JOIN on assets
                                            at read time. apy_pct is per (account,
                                            asset): same asset can yield differently
                                            in different accounts (Wise USD vs Chase
                                            USD, USDC at a yield protocol vs cold).
price_history (asset_id, as_of, price_usd, source)
fx_rates (ccy, as_of, rate_usd)
allocation_targets (id, category, market_mode, target_pct, notes)
                                         ← bull / crab / bear per category;
                                            sum per market_mode is enforced ≤ 100%
income (id, as_of, salary_usd, bonus_usd, taxes_usd, ...)
expenses (id, as_of, amount_usd, place, notes, category_id)
categories (id, name, parent_id, color, active)  ← hierarchical
labels (id, name, color, active)
expense_labels (expense_id, label_id)
settings (key, value, updated_at)        ← app-level prefs (display_currency, …)
transactions (...)                       ← schema present, light usage
```

## Decision-making endpoints

Beyond the standard CRUD, `nworth-web` exposes a layer of pre-computed insights that drive the Action Center:

```
GET /api/insights/summary          ← bundled payload powering /
GET /api/insights/drift            ← per-category drift in pp + USD
GET /api/insights/concentration    ← top-1 / top-3 / HHI
GET /api/insights/networth/deltas  ← 7d / 30d / 90d / YTD
GET /api/insights/actions          ← ranked action list, urgency 0-100
GET /api/insights/wealth           ← unified holdings for /wealth
GET /api/yield/summary             ← passive income aggregated from positions.apy_pct
GET /api/market/sentiment          ← 50/200 SMA per representative asset
GET /api/expenses/by-category      ← spending donut grouped by category
```

Action ranking combines drift × concentration × sentiment alignment × recency. Suppression thresholds (drift < 2pp or < $1,000, min trade $500/0.5% NW, top-1 < 25% concentration) are tuned to avoid noise.

## Pricefeed

`nworth-feed` is a separate binary that fetches market data and writes to the same SQLite. It's **not** required for the app to run — without it, the Action Center sentiment shows "Collecting data — 0/200 days" but everything else (snapshots, drift against targets, deltas) works fine.

What it does each cycle:
- Pulls crypto prices (CoinGecko) for assets with a `coingecko_id`
- Pulls stock prices (Yahoo) for assets with a `yahoo_ticker`
- Pulls FX rates for non-USD currencies in use
- Updates `assets.last_price` for live holdings (positions read it via a JOIN)
- Appends to `price_history` (used by the 50/200-day SMA logic)

Run it manually (`--once`), on a loop (`--loop SECONDS`), or schedule via cron / systemd / docker-compose.

## CLI

`nworth-cli` is a thin client over the REST API. Useful for scripting and quick edits without opening the browser.

```bash
nworth-cli accounts list
nworth-cli accounts create '{"name":"Example","type_code":"bank"}'
nworth-cli categories create '{"name":"Food","color":"#c0392b"}'
nworth-cli expenses create '{"as_of":"2026-04-26","amount_usd":42.50,"place":"Sushi","category_id":1}'
nworth-cli networth
nworth-cli help
```

The same `--url` flag points it at any deployment: `nworth-cli --url https://nworth.example.com accounts list`.

## Testing

The library is split (`lib.rs` + thin `main.rs`) so integration tests boot the whole Axum router against an in-memory SQLite.

```bash
cargo test --release          # all
cargo test --release --test rest_api    # REST CRUD lifecycle
cargo test --release --test api         # page renders, chart APIs
```

Today: **22 integration tests across `tests/api.rs` and `tests/rest_api.rs`** — page renders on empty + seeded DB, every REST resource's CRUD lifecycle, smart-delete fallback when references exist, insights summary returns correct totals.

## Configuration

Environment variables (see `.env.example`):

| Var | Default | Purpose |
|---|---|---|
| `BIND_ADDR` | `0.0.0.0:8080` | Listen address for `nworth-web` |
| `DATABASE_URL` | `sqlite://data/portfolio.db?mode=rwc` | SQLite path (shared by all three binaries) |
| `COINGECKO_API_KEY` | — | Optional; demo tier is fine. Used by `nworth-feed` |
| `HELIUS_RPC_URL` | — | Optional; for on-chain reads (Solana) |
| `RUST_LOG` | `info` | Log level |

## Roadmap

Tracked in [GitHub issues](https://github.com/zot24/nworth/issues). Active scopes:

- **Liability tracking** — make net worth truly net (assets − liabilities)
- **Expenses Phase 2/3/4** — budgets, recurring transactions, calendar / payee / period-comparison reports
- **Action Center Phase 2/3** — Cmd+K palette, detail drawer, optimistic updates, bulk select, keyboard shortcuts
- **Infrastructure** — `nworth-feed` scheduling in docker-compose, multi-binary Dockerfile, CI updates

## Project layout

```
.
├── Cargo.toml                ← three [[bin]] entries
├── Dockerfile + docker-compose.yml
├── migrations/               ← sqlx migrations (run on startup)
├── openapi.yaml              ← REST API spec, served at /api/docs
├── src/
│   ├── lib.rs                ← Axum router + state
│   ├── main.rs               ← nworth-web entry
│   ├── bin/
│   │   ├── pricefeed.rs      ← nworth-feed entry
│   │   └── cli.rs            ← nworth-cli entry
│   ├── config.rs, db.rs, error.rs
│   ├── models/
│   ├── routes/               ← page + JSON + REST handlers
│   └── services/             ← prices (CoinGecko/Yahoo), fx, solana
├── templates/                ← Askama HTML
├── static/
└── tests/                    ← integration tests
```

## License

Personal project. No license declared yet — if you fork, do so for your own use.
