-- SQLite migration for the log_entries table used by DatabaseSink.
--
-- Usage: copy this file to your active migrations directory with a versioned
-- prefix, e.g. `database/migrations/0001_create_log_entries.sql`, then
-- configure a "logs" connection in config/database.toml pointing to a SQLite
-- database. Enable the [[sinks]] database entry in config/logging.toml.
--
CREATE TABLE IF NOT EXISTS log_entries (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    level      TEXT    NOT NULL,
    target     TEXT    NOT NULL,
    message    TEXT    NOT NULL,
    span_name  TEXT,
    context    TEXT,   -- JSON serialized as a string
    request_id TEXT,
    logged_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);
