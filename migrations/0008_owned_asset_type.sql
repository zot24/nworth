-- New asset_type for owned-but-not-investment things (car, future house, art).
-- Used by the streamlined per-account "update value" flow on the Owned section
-- of /accounts: a single auto-provisioned asset of type 'owned' per account
-- holds the manual valuation snapshots. Keeps the snapshots schema unchanged
-- (asset_id remains NOT NULL) without overloading 'fiat' or 'asset'.

INSERT OR IGNORE INTO asset_types (code, label) VALUES ('owned', 'Owned Asset');
