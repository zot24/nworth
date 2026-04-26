-- Add market_mode (bull/crab/bear) to allocation_targets.
-- Existing rows become 'crab' targets. Duplicate for bull and bear.

-- Recreate table with new UNIQUE constraint (category + market_mode)
CREATE TABLE IF NOT EXISTS allocation_targets_new (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    category    TEXT    NOT NULL,
    market_mode TEXT    NOT NULL DEFAULT 'crab',
    target_pct  REAL    NOT NULL,
    notes       TEXT,
    updated_at  TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(category, market_mode)
);

-- Migrate existing rows as 'crab' targets
INSERT OR IGNORE INTO allocation_targets_new(category, market_mode, target_pct, notes, updated_at)
    SELECT category, 'crab', target_pct, notes, COALESCE(updated_at, CURRENT_TIMESTAMP)
    FROM allocation_targets;

-- Create bull copies (same as crab initially — user will adjust)
INSERT OR IGNORE INTO allocation_targets_new(category, market_mode, target_pct, notes)
    SELECT category, 'bull', target_pct, 'bull market targets'
    FROM allocation_targets;

-- Create bear copies
INSERT OR IGNORE INTO allocation_targets_new(category, market_mode, target_pct, notes)
    SELECT category, 'bear', target_pct, 'bear market targets'
    FROM allocation_targets;

DROP TABLE IF EXISTS allocation_targets;
ALTER TABLE allocation_targets_new RENAME TO allocation_targets;
