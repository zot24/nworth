-- 'credit' account_type for platform-specific prepaid balances: store credit
-- (REI Co-op dividend, returns), pre-loaded merchant accounts (Airbnb credit
-- you've topped up to capture promo offers), gift cards, vendor vouchers.
-- These look superficially like cash but have a critical difference: they're
-- non-transferable spend earmarked for a single merchant. Filing them under
-- 'cash' (Physical Cash) was a misclassification — physical cash is fungible
-- and walking-around; platform credit is sticky and merchant-locked.
--
-- Mostly pairs with role='operating' since you're not allocating it
-- strategically, but the role is independent of the type.

INSERT OR IGNORE INTO account_types (code, label) VALUES ('credit', 'Platform Credit');
