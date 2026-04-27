-- Conceptual rename: an account is a container, an asset is a thing of value.
-- "Car" was both — the account *and* the bridging asset row carried the same
-- name. We're moving to one "Physical Holdings" container that can hold many
-- owned-type assets (CAR, APARTMENT, WATCH, …), mirroring how a broker account
-- contains multiple stocks. Existing snapshot rows already point at this
-- account_id + the CAR asset_id, so the rename alone is sufficient — the new
-- owned-asset creation flow appends new asset rows under the same container.

UPDATE accounts
   SET name = 'Physical Holdings'
 WHERE is_investment = 0
   AND name = 'Car';
