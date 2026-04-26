-- Seed the enum-like reference tables.

INSERT OR IGNORE INTO account_types(code, label) VALUES
  ('broker',   'Broker'),
  ('bank',     'Bank'),
  ('cash',     'Physical Cash'),
  ('exchange', 'Crypto Exchange'),
  ('crypto',   'Crypto (aggregate)');

INSERT OR IGNORE INTO asset_types(code, label) VALUES
  ('crypto', 'Crypto'),
  ('stock',  'Stock / ETF'),
  ('stable', 'Stablecoin'),
  ('fiat',   'Fiat Currency');

INSERT OR IGNORE INTO risk_categories(code, label, sort_order) VALUES
  ('cat1_safe',   'Cat1 - Safe',        1),
  ('cat2_medium', 'Cat2 - Medium Risk', 2),
  ('cat3_high',   'Cat3 - High Risk',   3);

INSERT OR IGNORE INTO chains(code, label) VALUES
  ('bitcoin',  'Bitcoin'),
  ('ethereum', 'Ethereum'),
  ('solana',   'Solana'),
  ('cosmos',   'Cosmos'),
  ('kujira',   'Kujira');
