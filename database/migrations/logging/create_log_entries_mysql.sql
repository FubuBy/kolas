-- MySQL migration for the log_entries table used by DatabaseSink.
--
-- Usage: copy this file to your active migrations directory with a versioned
-- prefix, e.g. `database/migrations/0001_create_log_entries.sql`, then
-- configure a "logs" connection in config/database.toml pointing to a MySQL
-- database. Enable the [[sinks]] database entry in config/logging.toml.
--
CREATE TABLE IF NOT EXISTS log_entries (
    id         BIGINT       NOT NULL AUTO_INCREMENT PRIMARY KEY,
    level      VARCHAR(10)  NOT NULL,
    target     VARCHAR(255) NOT NULL,
    message    TEXT         NOT NULL,
    span_name  VARCHAR(255),
    context    JSON,
    request_id VARCHAR(64),
    logged_at  DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    INDEX idx_log_level     (level),
    INDEX idx_log_logged_at (logged_at),
    INDEX idx_log_request   (request_id)
);
