-- Seed expanded reference data.

INSERT OR IGNORE INTO account_types(code, label) VALUES ('asset', 'Physical Asset');

INSERT OR IGNORE INTO asset_types(code, label) VALUES ('nft', 'NFT');

INSERT OR IGNORE INTO allocation_targets(category, target_pct) VALUES
  ('stocks',          0.45),
  ('stable_yielding', 0.43),
  ('crypto',          0.05),
  ('cash',            0.05),
  ('car',             0.02);
