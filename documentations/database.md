# Work with a relational database

Kolas ships an async database layer built on top of **SQLx 0.8**. It supports multiple **named connections** declared in `config/database.toml`, with lazy pool initialization and a static `Database` facade that mirrors the `Config` one.

Supported drivers: **PostgreSQL**, **MySQL** (and forks: MariaDB, Percona), **SQLite**. MS SQL Server is intentionally not supported — see `dev_docs/database/improvements.md` (item 9) for the path forward if it's ever required.

## 1. Configure a connection

Drop entries into `config/database.toml`. The same shape works for all three drivers; pick `driver` accordingly.

```toml
default = "primary"
auto_migrate = false
migrations_path = "./database/migrations"

# MySQL / MariaDB / Percona
[connections.primary]
driver = "mysql"
host = "127.0.0.1"
port = 3306
database = "kolas"
username = "root"
password = ""
read = []

[connections.primary.pool]
max = 10
min = 1
acquire_timeout_ms = 5000
idle_timeout_ms = 600000
max_lifetime_ms = 1800000

# PostgreSQL — add as many connections as you need
[connections.analytics]
driver = "postgres"
host = "warehouse.internal"
port = 5432
database = "events"
username = "ro"
password = ""

# SQLite — file-based (use `?mode=rwc` so the file is created on first open)
[connections.cache]
driver = "sqlite"
url = "sqlite://./storage/cache.sqlite?mode=rwc"
```

For non-standard connection parameters (e.g. Postgres `sslmode=require`) — and, when going through `Database::any(...)`, for passwords containing special characters — set the explicit `url` field instead of `host`/`port`/`username`/`password`. The typed paths (`Database::postgres / mysql / sqlite`) pass credentials directly to SQLx connect options and need no URL-encoding.

You can also omit `port` entirely; SQLx will apply the driver default (5432 / 3306) when opening the connection.

Any value can be overridden by an environment variable using the convention from [Configuration](configuration.md):

| TOML path | ENV variable |
|---|---|
| `database.default` | `DATABASE__DEFAULT` |
| `database.auto_migrate` | `DATABASE__AUTO_MIGRATE` |
| `database.connections.primary.host` | `DATABASE__CONNECTIONS__PRIMARY__HOST` |
| `database.connections.primary.pool.max` | `DATABASE__CONNECTIONS__PRIMARY__POOL__MAX` |
| `database.connections.cache.url` | `DATABASE__CONNECTIONS__CACHE__URL` |

## 2. Get a pool in a controller

Three accessors cover three common needs:

```rust
use kolas::framework::database::{Connection, Database};

// Universal — sum-type. Pattern-match on the variant when needed.
let conn = Database::connection("primary").await?;
match conn {
    Connection::Postgres(pool) => { /* sqlx::Pool<Postgres> */ }
    Connection::MySql(pool)    => { /* sqlx::Pool<MySql>    */ }
    Connection::Sqlite(pool)   => { /* sqlx::Pool<Sqlite>   */ }
}

// Driver-agnostic — AnyPool. The URL scheme picks the driver at runtime.
// `query!` / `query_as!` are NOT available with AnyPool.
let any = Database::any("primary").await?;
sqlx::query("SELECT 1").execute(&any).await?;

// Typed — required if you want compile-time-checked queries via `query!`.
let pool = Database::mysql("primary").await?;
let users = sqlx::query_as::<_, (i64, String)>("SELECT id, name FROM users")
    .fetch_all(&pool)
    .await?;

// Shortcut for `Database::connection(<the configured default name>)`.
let conn = Database::default().await?;
```

A complete controller example:

```rust
use axum::Json;
use kolas::framework::database::Database;
use serde::Serialize;

#[derive(Serialize)]
pub struct UserListResponse {
    pub users: Vec<String>,
}

pub struct UsersController;

impl UsersController {
    pub async fn index() -> Result<Json<UserListResponse>, axum::http::StatusCode> {
        let pool = Database::mysql("primary")
            .await
            .map_err(|_| axum::http::StatusCode::SERVICE_UNAVAILABLE)?;

        let rows: Vec<(String,)> = sqlx::query_as("SELECT name FROM users")
            .fetch_all(&pool)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(Json(UserListResponse {
            users: rows.into_iter().map(|(n,)| n).collect(),
        }))
    }
}
```

## 3. Add a new connection

1. Add a `[connections.<name>]` section to `config/database.toml` with the appropriate `driver` and credentials.
2. Use it from code: `Database::connection("<name>")` (or the typed shortcut for the right driver).

No code changes in the framework, no registration step. New connections are picked up on the next process start.

## 4. Migrations

SQL files live in `database/migrations/` (configurable via `database.migrations_path`) and are processed by `sqlx::migrate::Migrator`. Each migration is a **reversible pair** named `<VERSION>_<description>.up.sql` / `<VERSION>_<description>.down.sql`. The `up` file applies the change; the `down` file reverts it and is required for rollback to work.

The `<VERSION>` is a UTC timestamp down to the millisecond — `YYYYMMDDHHMMSSmmm` (17 digits, e.g. `20260628161504914`). It's a contiguous digit string, so sqlx parses the whole prefix as the migration version and chronological order equals version order. Because it's clock-based rather than a shared counter, two people creating migrations on separate branches won't collide on the same `0001`.

### Create a migration

```bash
cargo run -- migration:create create_users
# Created ./database/migrations/20260628161504914_create_users.up.sql
# Created ./database/migrations/20260628161504914_create_users.down.sql
```

Fill in the generated files:

```sql
-- 20260628161504914_create_users.up.sql
CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT NOT NULL);
```

```sql
-- 20260628161504914_create_users.down.sql
DROP TABLE users;
```

### Run pending migrations

```bash
cargo run -- migration:migrate                      # against the default connection
cargo run -- migration:migrate --connection=analytics  # against a named connection
```

Equivalent paths from code or at boot:

- **At boot, automatically** — set `auto_migrate = true` in `database.toml` (default is `false`); after `Database::install_global()` the framework migrates the **default** connection plus every connection that declares its own `migrations_path` (see *Multiple databases* below).
- **Manually, from code** — `kolas::framework::database::migrate_default("./database/migrations").await?;`
- **Against a specific connection** — `migrate("analytics", "./database/migrations/mysql").await?;`

Repeated runs are idempotent thanks to the `_sqlx_migrations` tracking table.

### Roll back the last migration

```bash
cargo run -- migration:rollback                       # default connection
cargo run -- migration:rollback --connection=analytics # named connection
```

Each invocation reverts exactly one migration (the newest applied one) by running its `down.sql`. Run it again to step back further. If nothing has been applied yet it prints `Nothing to roll back.` and exits successfully. From code: `rollback_default("./database/migrations").await?;` or `rollback("analytics", "./database/migrations/mysql").await?;` for a specific connection.

### Multiple databases (Postgres + MySQL + …)

Migration state is tracked **per database** — the `_sqlx_migrations` table lives inside each target DB, so versions never collide across connections. To keep each database's *files* separate (and avoid running Postgres SQL against MySQL), give each connection its own `migrations_path` in `config/database.toml`:

```toml
default = "pg"
auto_migrate = true
migrations_path = "./database/migrations"   # global fallback for connections without their own

[connections.pg]
driver = "postgres"
url = "postgres://localhost/app"
migrations_path = "./database/migrations/postgres"

[connections.analytics]
driver = "mysql"
url = "mysql://localhost/analytics"
migrations_path = "./database/migrations/mysql"
```

- **Resolution order** for a connection's directory: its own `migrations_path` → the global `migrations_path` → the built-in `./database/migrations`. Connections without an override share the global directory (the single-database default), so existing setups are unchanged.
- **Targeting** a connection on the CLI: `--connection=<name>` on `migration:create`, `migration:migrate`, and `migration:rollback`. Omitting it uses the configured `default`. (`migration:create` without `--connection` writes to the global directory, since scaffolding files needs no live connection.)
- **`auto_migrate`** runs the default connection plus every connection that sets its own `migrations_path`; connections that neither are the default nor declare a path are skipped (the framework won't guess which dialect's files belong to them).

```bash
cargo run -- migration:create create_users --connection=pg
# Created ./database/migrations/postgres/20260628161504914_create_users.up.sql
# Created ./database/migrations/postgres/20260628161504914_create_users.down.sql

cargo run -- migration:migrate  --connection=analytics
cargo run -- migration:rollback --connection=pg
```

## 5. Lazy initialization

`Database::install_global()` does **not** open any sockets. The pool for a given connection name is opened on the **first call** to `Database::connection(name)` / `Database::any(name)` / `Database::<driver>(name)`. As a consequence:

- `cargo run` boots successfully even if the database server is currently down.
- The first request that actually touches the DB pays the connection-establishment cost.
- For aggressive warm-up, call the relevant accessor in `bootstrap::app::run()` after `install_global()`.

Architecture rationale, full configuration reference and a backlog of future improvements live in `dev_docs/architecture/database.md` and `dev_docs/database/improvements.md`.

[← Back to readme](../readme.md)
