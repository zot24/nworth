-- Add a 'wallet' account_type for self-custody crypto containers (Ledger,
-- Phantom, MetaMask, individual chain wallets). Until now the only crypto
-- type was 'crypto' which the original schema treated as an aggregate
-- bucket — fine for one-account-per-chain, wrong for users who track
-- multiple distinct wallets (each with its own private key, address,
-- security profile, and airdrop eligibility).
--
-- Companion to the existing 'exchange' code which covers CEXs (Gemini,
-- Binance, Kraken, …). Together: exchange = custodial, wallet = self-custody.

INSERT OR IGNORE INTO account_types (code, label) VALUES ('wallet', 'Crypto Wallet');
