-- Distinguish investment accounts (stocks, crypto, cash earning interest, brokerage) from
-- "owned" accounts (car, future house, art, electronics — physical things that contribute
-- to net worth but are not investments and should be excluded from drift/allocation math).
--
-- Default is 1 (investment) so all existing accounts keep their current behavior. Only
-- accounts explicitly marked is_investment=0 are treated as owned.

ALTER TABLE accounts ADD COLUMN is_investment INTEGER NOT NULL DEFAULT 1;

CREATE INDEX IF NOT EXISTS idx_accounts_is_investment ON accounts(is_investment);
