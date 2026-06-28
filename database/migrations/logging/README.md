# Log Entries Migration Templates

These SQL files create the `log_entries` table required by `DatabaseSink`.
They are **templates** — the `sqlx` Migrator only scans the parent directory
(`database/migrations/`), so files in this subdirectory are never run automatically.

## How to use

1. Choose the file matching your database driver:
   - `create_log_entries_sqlite.sql`
   - `create_log_entries_postgres.sql`
   - `create_log_entries_mysql.sql`

2. Copy it to the active migrations directory with a versioned prefix, for example:

   ```
   cp database/migrations/logging/create_log_entries_sqlite.sql \
      database/migrations/0001_create_log_entries.sql
   ```

3. Add a `logs` connection to `config/database.toml`:

   ```toml
   [connections.logs]
   driver   = "sqlite"
   url      = "sqlite://storage/logs.db"
   ```

4. Enable the database sink in `config/logging.toml`:

   ```toml
   [[sinks]]
   type       = "database"
   level      = "warn"
   connection = "logs"
   table      = "log_entries"
   ```

5. Run migrations (`cargo run -- migrate` or via `auto_migrate = true` in `config/database.toml`).

## Table schema

| Column      | Type         | Notes                          |
|-------------|--------------|--------------------------------|
| id          | integer/serial | auto-increment primary key   |
| level       | text/varchar   | ERROR, WARN, INFO, DEBUG, TRACE |
| target      | text/varchar   | e.g. `myapp::controllers::users` |
| message     | text           | log message body               |
| span_name   | text (nullable)| active span name, if any       |
| context     | text/json      | structured fields as JSON      |
| request_id  | text (nullable)| for future request correlation |
| logged_at   | timestamp      | event timestamp (UTC)          |
