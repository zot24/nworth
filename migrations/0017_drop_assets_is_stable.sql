-- Remove assets.is_stable. The boolean is redundant with type_code='stable'
-- (which already encodes "this is a stablecoin"). Grepping every code path
-- confirms is_stable is stored + displayed but never used in math, filtering,
-- or aggregation — same dead-column profile as target_pct (migration 0016).
--
-- Cash page filters by type_code IN ('fiat','stable'), drift's
-- stable_yielding category aggregates by type_code='stable'. Nothing reads
-- assets.is_stable for behavior. Dropping it removes a misleading UI control.

ALTER TABLE assets DROP COLUMN is_stable;
