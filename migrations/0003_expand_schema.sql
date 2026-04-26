-- =============================================================
-- Schema expansion: expenses, income, loans, allocation targets.
-- =============================================================

PRAGMA foreign_keys = ON;

-- ---------- Expenses (monthly) ----------

CREATE TABLE IF NOT EXISTS expenses (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    as_of      TEXT    NOT NULL,
    amount_usd REAL    NOT NULL DEFAULT 0,
    place      TEXT,
    notes      TEXT,
    source     TEXT    DEFAULT 'manual',
    UNIQUE(as_of)
);

CREATE INDEX IF NOT EXISTS idx_expenses_as_of ON expenses(as_of);

-- ---------- Income (monthly) ----------

CREATE TABLE IF NOT EXISTS income (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    as_of        TEXT    NOT NULL,
    salary_usd   REAL    NOT NULL DEFAULT 0,
    per_year_usd REAL    NOT NULL DEFAULT 0,
    bonus_usd    REAL    NOT NULL DEFAULT 0,
    taxes_usd    REAL    NOT NULL DEFAULT 0,
    company      TEXT,
    source       TEXT    DEFAULT 'manual',
    UNIQUE(as_of)
);

CREATE INDEX IF NOT EXISTS idx_income_as_of ON income(as_of);

-- ---------- Loans ----------

CREATE TABLE IF NOT EXISTS loans (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT    NOT NULL UNIQUE,
    principal  REAL    NOT NULL,
    rate_pct   REAL    NOT NULL,
    start_date TEXT,
    notes      TEXT,
    active     INTEGER NOT NULL DEFAULT 1,
    created_at TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS loan_allocations (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    loan_id    INTEGER NOT NULL REFERENCES loans(id) ON DELETE CASCADE,
    asset_id   INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    target_pct REAL,
    invested   REAL,
    value_usd  REAL,
    gain_pct   REAL,
    as_of      TEXT    NOT NULL,
    UNIQUE(loan_id, asset_id, as_of)
);

CREATE INDEX IF NOT EXISTS idx_loan_alloc_loan ON loan_allocations(loan_id);
CREATE INDEX IF NOT EXISTS idx_loan_alloc_as_of ON loan_allocations(as_of);

-- ---------- Allocation targets (category-level) ----------

CREATE TABLE IF NOT EXISTS allocation_targets (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    category   TEXT    NOT NULL UNIQUE,
    target_pct REAL    NOT NULL,
    notes      TEXT,
    updated_at TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP
);
