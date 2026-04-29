-- Remove assets.target_pct — dead column.
--
-- The portfolio's allocation targets are keyed on *category* (stocks,
-- crypto, stable_yielding, cash) under each market_mode (bull/crab/bear)
-- via the allocation_targets table introduced in migration 0005. Drift,
-- rebalance suggestions, and the targets editor all read from there.
--
-- assets.target_pct was the original per-asset shape from migration 0001
-- but no live code path uses it for math anymore — it just clutters the
-- /data?tab=assets editor and the AssetIn/AssetOut REST shape.

ALTER TABLE assets DROP COLUMN target_pct;
