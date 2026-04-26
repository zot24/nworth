-- =============================================================
-- Portfolio tracker schema (v1)
-- Long-format, normalized. One row per (date, account, asset).
-- Replaces the wide-format xlsx.
-- =============================================================

PRAGMA foreign_keys = ON;

-- ---------- Reference tables ----------

CREATE TABLE IF NOT EXISTS account_types (
    code  TEXT PRIMARY KEY,       -- broker | bank | exchange | cash | crypto
    label TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS asset_types (
    code  TEXT PRIMARY KEY,       -- crypto | stock | stable | fiat
    label TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS risk_categories (
    code       TEXT PRIMARY KEY,  -- cat1_safe | cat2_medium | cat3_high
    label      TEXT NOT NULL,
    sort_order INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS chains (
    code  TEXT PRIMARY KEY,
    label TEXT NOT NULL
);

-- ---------- Core tables ----------

-- An account can hold multiple assets (currencies, tokens, tickers).
-- Example: one multi-currency bank account holds USD + EUR + GBP. One brokerage
-- account holds USD cash + a few stock tickers.
CREATE TABLE IF NOT EXISTS accounts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL UNIQUE,           -- e.g. "MyBank", "MyBroker", "Crypto"
    type_code   TEXT    NOT NULL REFERENCES account_types(code),
    institution TEXT,
    chain_code  TEXT    REFERENCES chains(code),   -- optional, for on-chain accounts (e.g. solana, ethereum)
    active      INTEGER NOT NULL DEFAULT 1,
    notes       TEXT,
    created_at  TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_accounts_type ON accounts(type_code);
CREATE INDEX IF NOT EXISTS idx_accounts_active ON accounts(active);

CREATE TABLE IF NOT EXISTS assets (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol       TEXT    NOT NULL,                 -- BTC, ETH, VOO, USDC, USD, EUR, PYG
    name         TEXT,
    type_code    TEXT    NOT NULL REFERENCES asset_types(code),
    chain_code   TEXT    REFERENCES chains(code),
    risk_code    TEXT    REFERENCES risk_categories(code),
    coingecko_id TEXT,
    yahoo_ticker TEXT,
    target_pct   REAL,
    is_stable    INTEGER NOT NULL DEFAULT 0,
    active       INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(symbol, type_code)
);

CREATE INDEX IF NOT EXISTS idx_assets_type ON assets(type_code);
CREATE INDEX IF NOT EXISTS idx_assets_chain ON assets(chain_code);
CREATE INDEX IF NOT EXISTS idx_assets_active ON assets(active);

-- Snapshots: time-series balance state. Everything on the dashboard derives from these.
CREATE TABLE IF NOT EXISTS snapshots (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    as_of      TEXT    NOT NULL,                    -- ISO date
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    asset_id   INTEGER NOT NULL REFERENCES assets(id)   ON DELETE CASCADE,
    quantity   REAL    NOT NULL DEFAULT 0,          -- in asset's native units
    price_usd  REAL,                                -- unit price at as_of
    value_usd  REAL    NOT NULL DEFAULT 0,          -- quantity * price, denormalized
    source     TEXT    DEFAULT 'manual',
    UNIQUE(as_of, account_id, asset_id)
);

CREATE INDEX IF NOT EXISTS idx_snapshots_as_of ON snapshots(as_of);
CREATE INDEX IF NOT EXISTS idx_snapshots_account ON snapshots(account_id);
CREATE INDEX IF NOT EXISTS idx_snapshots_asset ON snapshots(asset_id);

-- Current positions: cached view of most recent snapshot per (account, asset).
CREATE TABLE IF NOT EXISTS positions (
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    asset_id   INTEGER NOT NULL REFERENCES assets(id)   ON DELETE CASCADE,
    quantity   REAL    NOT NULL DEFAULT 0,
    avg_cost   REAL,
    last_price REAL,
    value_usd  REAL    NOT NULL DEFAULT 0,
    as_of      TEXT    NOT NULL,
    PRIMARY KEY (account_id, asset_id)
);

-- Transaction ledger (schema ready, unused in v1).
CREATE TABLE IF NOT EXISTS transactions (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    ts         TEXT    NOT NULL,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    asset_id   INTEGER NOT NULL REFERENCES assets(id)   ON DELETE CASCADE,
    kind       TEXT    NOT NULL CHECK (kind IN (
        'buy','sell','deposit','withdraw','dividend','fee','interest','transfer_in','transfer_out'
    )),
    quantity   REAL    NOT NULL,
    price_usd  REAL,
    fee_usd    REAL    DEFAULT 0,
    counterparty_account_id INTEGER REFERENCES accounts(id),
    notes      TEXT,
    created_at TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_tx_ts ON transactions(ts);
CREATE INDEX IF NOT EXISTS idx_tx_account ON transactions(account_id);
CREATE INDEX IF NOT EXISTS idx_tx_asset ON transactions(asset_id);

-- Price history cache.
CREATE TABLE IF NOT EXISTS price_history (
    asset_id  INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    as_of     TEXT    NOT NULL,
    price_usd REAL    NOT NULL,
    source    TEXT    NOT NULL,
    PRIMARY KEY (asset_id, as_of)
);

-- FX rates (USD base).
CREATE TABLE IF NOT EXISTS fx_rates (
    ccy      TEXT    NOT NULL,
    as_of    TEXT    NOT NULL,
    rate_usd REAL    NOT NULL,
    source   TEXT    NOT NULL DEFAULT 'exchangerate.host',
    PRIMARY KEY (ccy, as_of)
);
