-- Move "current price" from the positions cache onto the asset itself.
--
-- Pre-refactor: pricefeed wrote the latest price into positions.last_price for
-- every position row referencing a given asset (BTC at Robinhood + Gemini +
-- Ledger = 3 identical writes per cycle). positions.value_usd was the
-- denormalized product. Risks: redundant storage, inconsistency window if the
-- pricefeed crashed mid-loop, and reads couldn't include unheld assets.
--
-- Post-refactor: assets.last_price is the single source of truth for an asset's
-- current price. Pricefeed writes one row per asset per cycle. Read paths
-- compute value on the fly via JOIN: positions.quantity * assets.last_price.
-- positions.last_price + positions.value_usd are dropped (data was always
-- derivable; nothing of value is lost).

ALTER TABLE assets ADD COLUMN last_price REAL;
ALTER TABLE assets ADD COLUMN last_price_as_of TEXT;

-- Backfill from price_history if any exists.
UPDATE assets SET
    last_price = (SELECT ph.price_usd FROM price_history ph
                   WHERE ph.asset_id = assets.id
                   ORDER BY ph.as_of DESC LIMIT 1),
    last_price_as_of = (SELECT ph.as_of FROM price_history ph
                         WHERE ph.asset_id = assets.id
                         ORDER BY ph.as_of DESC LIMIT 1);

-- Drop the now-redundant denormalization on positions.
ALTER TABLE positions DROP COLUMN last_price;
ALTER TABLE positions DROP COLUMN value_usd;
