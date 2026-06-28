-- PostgreSQL migration for the log_entries table used by DatabaseSink.
--
-- Usage: copy this file to your active migrations directory with a versioned
-- prefix, e.g. `database/migrations/0001_create_log_entries.sql`, then
-- configure a "logs" connection in config/database.toml pointing to a Postgres
-- database. Enable the [[sinks]] database entry in config/logging.toml.
--
-- Note: context is TEXT (not JSONB) for consistency with SQLite and MySQL TEXT,
-- because DatabaseSink binds context as a serialized JSON string via AnyPool.
-- If you want JSONB in Postgres, add a `context::jsonb` cast in your queries.
--
CREATE TABLE IF NOT EXISTS log_entries (
    id         BIGSERIAL    PRIMARY KEY,
    level      VARCHAR(10)  NOT NULL,
    target     VARCHAR(255) NOT NULL,
    message    TEXT         NOT NULL,
    span_name  VARCHAR(255),
    context    TEXT,
    request_id VARCHAR(64),
    logged_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_log_level     ON log_entries (level);
CREATE INDEX IF NOT EXISTS idx_log_logged_at ON log_entries (logged_at);
CREATE INDEX IF NOT EXISTS idx_log_request   ON log_entries (request_id);
