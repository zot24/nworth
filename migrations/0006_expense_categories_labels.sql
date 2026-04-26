-- Phase 1 of expense feature: categories (with parent-child hierarchy) and labels (orthogonal tagging).
-- Scope: expenses only for now. Income side will mirror this in a future migration if needed.

CREATE TABLE IF NOT EXISTS categories (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    name      TEXT NOT NULL,
    parent_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    color     TEXT,
    active    INTEGER NOT NULL DEFAULT 1,
    UNIQUE(name, parent_id)
);

CREATE INDEX IF NOT EXISTS idx_categories_parent ON categories(parent_id);

CREATE TABLE IF NOT EXISTS labels (
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    name   TEXT NOT NULL UNIQUE,
    color  TEXT,
    active INTEGER NOT NULL DEFAULT 1
);

ALTER TABLE expenses ADD COLUMN category_id INTEGER REFERENCES categories(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_expenses_category ON expenses(category_id);

CREATE TABLE IF NOT EXISTS expense_labels (
    expense_id INTEGER NOT NULL REFERENCES expenses(id) ON DELETE CASCADE,
    label_id   INTEGER NOT NULL REFERENCES labels(id)   ON DELETE CASCADE,
    PRIMARY KEY (expense_id, label_id)
);

CREATE INDEX IF NOT EXISTS idx_expense_labels_label ON expense_labels(label_id);
