-- =============================================================
-- Add per-position APY (annual yield rate, percent).
--
-- Yield is a property of the (account, asset) pair, not of the asset alone:
-- USD held in Wise yields ~4%, the same USD held in Chase yields 0%; USDC at
-- a yield protocol earns, USDC in a cold wallet doesn't. Storing the rate on
-- positions captures that.
--
-- DEFAULT 0 keeps existing rows intact; the user fills the rate in via the
-- positions editor on /data?tab=positions.
-- =============================================================

ALTER TABLE positions ADD COLUMN apy_pct REAL NOT NULL DEFAULT 0;
