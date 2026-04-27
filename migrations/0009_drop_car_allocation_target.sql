-- The 'car' rows in allocation_targets were seeded when Car was treated as a
-- tradable category with its own slice of the portfolio. After migration 0007,
-- owned accounts (is_investment=0) are excluded from drift/allocation math
-- entirely (see insights.rs::compute_drift, ::category_values). Any leftover
-- 'car' target row would be picked up as a phantom 5th investment category
-- and skew abs_drift_pp / category renders.

DELETE FROM allocation_targets WHERE category = 'car';
