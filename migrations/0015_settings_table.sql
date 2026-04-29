-- Generic key/value settings store for app-level preferences. Starts with one
-- row — display_currency — but the schema is intentionally key/value so future
-- toggles (default_theme, default_landing_tab, …) don't need new migrations.
--
-- display_currency holds an ISO 4217-ish code that the UI uses to format and
-- convert the USD-denominated stored values for display. Defaults to 'USD'
-- (no conversion). Wiring conversion through every template is a separate
-- follow-up; this migration only stores the preference.

CREATE TABLE IF NOT EXISTS settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO settings(key, value) VALUES ('display_currency', 'USD');
