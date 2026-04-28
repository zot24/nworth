-- Replace the boolean is_investment flag with a 3-way `role` enum.
--
-- The boolean only had two meanings — "in drift math" vs "not in drift math" —
-- which collapsed two genuinely different things:
--
--   investment  = strategically allocated capital (Robinhood, Cash USD bucket,
--                 crypto exchange) — counts in drift / target-rebalancing math.
--   operating   = day-to-day liquid cash (Citi, Chase) — counts in net worth
--                 but NOT drift; you're not allocating it on purpose.
--   property    = physical things of value (Car, Apartment, Watches) — counts
--                 in net worth but NOT drift; not even a financial asset.
--
-- All three count toward total net worth. Only `role = 'investment'` participates
-- in drift / allocation / concentration / market-sentiment calculations.
--
-- Backfill mapping: is_investment=1 → 'investment', is_investment=0 → 'property'
-- (the only is_investment=0 case before this migration was the Property/Car
-- container; the user reclassifies operating accounts after this migration runs).

ALTER TABLE accounts ADD COLUMN role TEXT NOT NULL DEFAULT 'investment'
    CHECK (role IN ('investment', 'operating', 'property'));

UPDATE accounts
   SET role = CASE WHEN is_investment = 1 THEN 'investment' ELSE 'property' END;

DROP INDEX IF EXISTS idx_accounts_is_investment;
ALTER TABLE accounts DROP COLUMN is_investment;
CREATE INDEX IF NOT EXISTS idx_accounts_role ON accounts(role);
