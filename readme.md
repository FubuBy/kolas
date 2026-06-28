# Kolas — A Lightweight Rust Web Framework on Top of Axum

Kolas is a lightweight Rust web framework that wraps [Axum](https://github.com/tokio-rs/axum) with an opinionated, MVC-style project layout and a fluent routing API. The goal is to give Rust developers a structured, ready-to-use project layout (controllers, routes, bootstrap) so they can stand up a service in minutes without wiring boilerplate by hand.

The framework core lives under `src/framework/` and is designed to be extracted into a standalone crate later. Application code (your controllers, your routes) lives under `src/app/` and `src/routes/`, following the convention of separating vendor framework code from application code.

## Status

Bootstrap iteration. Currently included:

- Async HTTP server based on Axum 0.8 and Tokio.
- Fluent `Route` builder facade (`Route::new().get(...).post(...).into_router()`).
- Controllers as Rust structs with associated `async fn` handlers.
- A working sample endpoint: `GET /hello` returning `{"payload": "Hello world"}`.
- File-based configuration with environment-variable overrides (`config/*.toml` + `.env`).
- HTTP: `Route::middleware` / `Route::route_middleware` for the framework `Middleware` trait (application code, e.g. `TrimStrings`), and `Route::layer` for Tower / **tower-http** layers. Sample `TrimStrings` trims JSON, form-urlencoded, and query strings.
- Default **tower-http** stack is chained on the same `Route` builder: **trace**, permissive **CORS**, **compression** (gzip / brotli / zstd / deflate). Subscriber: `bootstrap::telemetry::Telemetry::init()` from `bootstrap::app::run()`; tune with `RUST_LOG` (see `.env.example`).
- Relational database layer on top of **SQLx 0.8**: multiple named connections in `config/database.toml`, lazy pool initialization, static `Database` facade with both an `AnyPool` path and typed `Pool<Postgres|MySql|Sqlite>` accessors, optional auto-migrate from `database/migrations/`.
- Console command layer: `ConsoleKernel` dispatches CLI arguments to `Command` trait implementations; built-in `serve` and `migrate` commands; register your own commands in `src/bootstrap/console.rs`.

Out of scope for now (planned): global error handler, resource routes.

## Requirements

- Rust toolchain compatible with Rust **edition 2024** (`rustc 1.94+` recommended).
- Cargo.

## Quick start

```bash
cargo run                     # default command → serve (same as before)
cargo run -- serve            # explicit: start the HTTP server
cargo run -- migration:migrate # run pending database migrations
cargo run -- test Alice       # example custom command → Hello, Alice!
cargo run -- help             # list all registered commands
# Listening on http://127.0.0.1:3000
curl http://127.0.0.1:3000/hello
# {"payload":"Hello world"}

# Optional: quieter logs (default subscriber uses RUST_LOG or info + tower_http=trace)
RUST_LOG=warn cargo run
```

Run tests:

```bash
cargo test
```

## Project structure (current)

```
src/
├── main.rs                                       # Tokio runtime entry point
├── lib.rs                                        # Library crate root: declares public modules
├── bootstrap/
│   ├── app.rs                                    # run(): telemetry, config install, HttpServer::run
│   ├── console.rs                                # run(): bootstrap + ConsoleKernel with registered commands
│   ├── server.rs                                 # HttpServer — bind + axum::serve from Config
│   └── telemetry.rs                              # Telemetry::init() — tracing subscriber + RUST_LOG
├── framework/                                    # Framework core (future standalone crate)
│   ├── config/                                   # Configuration loader (TOML + env overrides)
│   │   ├── config.rs                             # Config struct + static facade
│   │   └── error.rs                              # Typed loader errors
│   ├── console/                                  # Console command layer
│   │   └── command.rs                            # Command trait + BoxFuture type alias; ConsoleKernel in mod.rs
│   ├── database/                                 # SQLx-based DB layer with named connections
│   │   ├── config.rs                             # DatabaseConfig, ConnectionConfig, DriverKind
│   │   ├── error.rs                              # DatabaseError
│   │   ├── manager.rs                            # Database + Connection enum + static facade
│   │   └── migrate.rs                            # sqlx::Migrator runner
│   ├── http/
│   │   └── middleware/                           # Middleware trait + Axum adapters
│   └── routing/
│       └── route.rs                              # Route builder facade over axum::Router
├── routes/
│   └── api.rs                                    # API route table — register your routes here
└── app/
    ├── console/
    │   └── commands/
    │       └── test_command.rs                   # Example custom command (greet [name])
    └── http/
        ├── controllers/
        │   └── hello_world_controller.rs         # Your controllers go in this directory
        └── middleware/
            └── trim_strings.rs                   # Example TrimStrings middleware

config/                                           # Application configuration (TOML files)
├── app.toml
└── database.toml

database/                                         # Schema and data files
└── migrations/                                   # Versioned SQL migrations (sqlx)

.env                                              # Local environment overrides (gitignored)
.env.example                                      # Committed template for required env vars

tests/
├── feature/                                      # Component / integration tests (HTTP, controllers, end-to-end)
│   ├── mod.rs                                    # entry: declares submodule tests
│   ├── config_loads_from_directory.rs
│   ├── hello_world_controller.rs
│   └── trim_strings.rs
└── unit/                                         # Focused unit tests of small components
    ├── mod.rs                                    # entry: declares submodule tests
    ├── config.rs
    ├── hello_payload.rs
    └── trim_strings.rs
```

---

# Instructions

## 1. Add a controller and register its route

This is the most common day-to-day task. It takes three steps.

### 1.1. Create the controller file

Create a new file `src/app/http/controllers/<name>_controller.rs`. A controller is a unit struct with one or more associated `async fn` handlers. Each handler returns an Axum-compatible response — most commonly `axum::Json<T>` for a JSON payload, where `T` derives `serde::Serialize`.

Example: `src/app/http/controllers/users_controller.rs`

```rust
use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct UserListResponse {
    pub users: Vec<String>,
}

pub struct UsersController;

impl UsersController {
    pub async fn index() -> Json<UserListResponse> {
        Json(UserListResponse {
            users: vec!["alice".into(), "bob".into()],
        })
    }

    pub async fn show() -> Json<&'static str> {
        Json("user details")
    }
}
```

### 1.2. Expose the controller from the controllers module

Open `src/app/http/controllers/mod.rs` and add two lines: declare the new module and re-export the controller struct.

```rust
pub mod hello_world_controller;
pub mod users_controller;          // <-- added

pub use hello_world_controller::HelloWorldController;
pub use users_controller::UsersController;   // <-- added
```

The `pub use` re-export lets you import the controller with the short path `crate::app::http::controllers::UsersController` instead of the longer `crate::app::http::controllers::users_controller::UsersController`.

### 1.3. Register the route in `src/routes/api.rs`

Open `src/routes/api.rs` and chain new routes onto the `Route` builder using the appropriate HTTP verb method (`get`, `post`, `put`, `patch`, `delete`).

```rust
use axum::Router;

use crate::app::http::controllers::{HelloWorldController, UsersController};
use crate::framework::routing::Route;

pub fn routes() -> Router {
    Route::new()
        .get("/hello", HelloWorldController::index)
        .get("/users", UsersController::index)        // <-- added
        .get("/users/:id", UsersController::show)     // <-- added
        .into_router()
}
```

That's it. Run `cargo run` and `curl http://127.0.0.1:3000/users` will hit your new handler.

#### Available verbs on `Route`

| Method | HTTP verb |
|---|---|
| `.get(path, handler)` | `GET` |
| `.post(path, handler)` | `POST` |
| `.put(path, handler)` | `PUT` |
| `.patch(path, handler)` | `PATCH` |
| `.delete(path, handler)` | `DELETE` |

Each method takes the path string and a handler — typically `SomeController::some_method`. The handler signature must satisfy Axum's `Handler` trait; in practice, any `async fn` returning an `IntoResponse` (e.g. `Json<T>`, `String`, `(StatusCode, Json<T>)`) will work.

## 2. Configure the application

Configuration is split into multiple TOML files under `config/`, each becoming a top-level namespace. Values are read from code with a static facade `Config::get(path, default)`, where `path` is a dot-notation address into the merged configuration tree. Any value can be overridden by an environment variable (or `.env` entry) without changing the TOML file.

### 2.1. Add a new config file

Drop a `*.toml` file into `config/`. The filename (without extension) becomes the **first segment** of every key inside it. No code changes, no struct registration — your values are immediately addressable.

Example: `config/payments.toml`

```toml
default = "stripe"

[providers.stripe]
secret = ""
public = ""
webhook_path = "/webhooks/stripe"

[providers.paypal]
client_id = ""
client_secret = ""
sandbox = true
```

The values above become accessible as:

- `payments.default`
- `payments.providers.stripe.secret`
- `payments.providers.stripe.webhook_path`
- `payments.providers.paypal.sandbox`
- …

### 2.2. Read values from code

Configuration is loaded once at startup in `src/bootstrap/app.rs` (`Config::load("config")?.install_global()`). After that, you can read values from anywhere:

```rust
use crate::framework::config::Config;

let provider: String = Config::get("payments.default", "stripe".to_string());
let secret: String   = Config::get("payments.providers.stripe.secret", String::new());
let sandbox: bool    = Config::get("payments.providers.paypal.sandbox", true);
let port: u16        = Config::get("app.port", 3000);
```

The default is returned in three cases: the path does not exist, the value type does not match `T`, or the section file is absent. If you need to distinguish "missing" from "default", use `Config::try_get`:

```rust
match Config::try_get::<String>("payments.providers.stripe.secret") {
    Some(s) if !s.is_empty() => { /* configured */ }
    _ => panic!("payments.providers.stripe.secret is required"),
}
```

To check existence without reading the value:

```rust
if Config::has("cache.stores.redis") {
    // ...
}
```

### 2.3. Read a whole subtree as a typed struct

When a section has stabilised, it is often more convenient to deserialise the whole subtree into a struct. `Config::try_get` works with any `T: serde::Deserialize`, including nested structs and `HashMap`s:

```rust
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct StripeConfig {
    secret: String,
    public: String,
    webhook_path: String,
}

#[derive(Deserialize)]
struct PaymentsConfig {
    default: String,
    providers: HashMap<String, toml::Value>,
}

let stripe: StripeConfig =
    Config::try_get("payments.providers.stripe").expect("stripe section is required");

let payments: PaymentsConfig =
    Config::try_get("payments").expect("payments section is required");
```

This is the recommended pattern for code that lives inside the framework or a stable service: typed structs catch missing or mistyped values at load time instead of later at the call site.

### 2.4. Override values with environment variables

Any value can be overridden by an environment variable (or by a line in `.env`) using the schema:

```
<SECTION>__<KEY>__<NESTED_KEY>__...=value
```

Rules:

- **Double underscore `__`** separates levels of nesting.
- **Single underscore `_`** stays as part of the key name (so `retry_after` is one key, not two).
- The first segment **must** match the name of an existing TOML file in `config/` — otherwise the override is ignored. This prevents random environment variables (`PATH`, `SHELL`, …) from accidentally creating phantom config sections.
- Values are auto-parsed: `"true"`/`"false"` become `bool`, `"42"` becomes integer, `"3.14"` becomes float, anything else stays a string.

Mapping table:

| TOML path | ENV variable |
|---|---|
| `app.host` | `APP__HOST` |
| `app.port` | `APP__PORT` |
| `database.connections.primary.host` | `DATABASE__CONNECTIONS__PRIMARY__HOST` |
| `cache.stores.redis.port` | `CACHE__STORES__REDIS__PORT` |
| `queue.connections.redis.retry_after` | `QUEUE__CONNECTIONS__REDIS__RETRY_AFTER` |
| `payments.providers.stripe.secret` | `PAYMENTS__PROVIDERS__STRIPE__SECRET` |

### 2.5. Use `.env` for local development

For local development, put overrides into a `.env` file at the project root. The file is loaded automatically by `Config::load(...)` via [`dotenvy`](https://crates.io/crates/dotenvy):

```bash
APP__NAME=Kolas
APP__DEBUG=true
APP__HOST=127.0.0.1
APP__PORT=3000

DATABASE__CONNECTIONS__PRIMARY__HOST=127.0.0.1
DATABASE__CONNECTIONS__PRIMARY__PORT=5432
DATABASE__CONNECTIONS__PRIMARY__PASSWORD=

PAYMENTS__PROVIDERS__STRIPE__SECRET=sk_test_local
```

`.env` is git-ignored. A committed template lives in `.env.example` — keep it in sync whenever you add a new environment variable so other developers know what they need to set.

In production, set the same variables through your process manager (systemd, Docker, Kubernetes), not through a file.

### 2.6. Resolution order

When the same key is set in multiple places, the last source wins:

1. **TOML file** (`config/<section>.toml`) — base value.
2. **`.env` file** at project root — local override.
3. **Process environment variable** — final override (production deployments, CI, ad-hoc runs like `APP__PORT=8080 cargo run`).

### 2.7. Static facade vs. instance API

For day-to-day application code, prefer the static facade (`Config::get`, `Config::try_get`, `Config::has`) — it requires no plumbing.

The instance API (`cfg.value(...)`, `cfg.try_value(...)`, `cfg.has_key(...)`) is exposed for tests and dependency-injection scenarios where you want to build a `Config` programmatically without touching the global singleton:

```rust
let cfg = Config::from_sections([
    ("app", r#"name = "Test""#),
])
.unwrap();
assert_eq!(cfg.value::<String>("app.name", "x".into()), "Test");
```

`Config::from_sections` does not scan the filesystem and does not apply environment overrides — it is purely an in-memory builder.

## 3. Write a test for your controller

Tests are split into two directories under `tests/`:

| Directory | Purpose | Typical content |
|---|---|---|
| `tests/feature/` | Component / integration tests | Calling controller handlers, asserting HTTP-shape responses, end-to-end flows |
| `tests/unit/` | Focused unit tests | Pure logic of small components: serialization, validation, helpers |

Each directory has a `mod.rs` entry file (registered as a Cargo test target via `[[test]]` in `Cargo.toml`) that declares its sub-modules. Adding a new test = adding the file plus one `mod` line.

### Adding a feature test

1. Create `tests/feature/<name>.rs`. Example: `tests/feature/users_controller.rs`:

   ```rust
   use kolas::app::http::controllers::UsersController;

   #[tokio::test]
   async fn index_returns_user_list() {
       let response = UsersController::index().await;
       assert_eq!(response.0.users, vec!["alice", "bob"]);
   }
   ```

2. Open `tests/feature/mod.rs` and append:

   ```rust
   mod users_controller;
   ```

### Adding a unit test

1. Create `tests/unit/<name>.rs`. Example: `tests/unit/some_helper.rs`:

   ```rust
   use kolas::framework::routing::Route;

   #[test]
   fn route_default_is_equivalent_to_new() {
       let _ = Route::default().into_router();
       let _ = Route::new().into_router();
   }
   ```

2. Open `tests/unit/mod.rs` and append:

   ```rust
   mod some_helper;
   ```

### Running tests

```bash
cargo test                   # all tests (both binaries)
cargo test --test feature    # only feature tests
cargo test --test unit       # only unit tests
```

### A note on Rust testing conventions

In Rust the term "unit test" traditionally means a test colocated with the code under test, inside the source file via `#[cfg(test)] mod tests { ... }`. Tests under `tests/` are technically "integration tests" because each compiles as a separate crate that sees only the **public** API of the `kolas` library. The `tests/unit/` and `tests/feature/` split adopted here is a project convention borrowed from PHPUnit-style organisation: it groups tests by their **scope and intent** (focused-component vs. full-flow), not by Cargo's underlying compilation model. Both directories sit at integration-test level and have full access to `kolas::*` public items.

## 4. Change the server bind address

The listener reads `app.host` and `app.port` from configuration. To change the bind address, set them in `config/app.toml`:

```toml
host = "0.0.0.0"
port = 8080
```

…or override at runtime without touching files:

```bash
APP__HOST=0.0.0.0 APP__PORT=8080 cargo run
```

Both forms are equivalent; the environment variable wins if both are set. See [section 2](#2-configure-the-application) for the full configuration model.

## 5. Add a new module to the framework core

If you want to extend the framework itself (e.g. add a request-validation layer, an exception handler), create the new sub-module under `src/framework/` and declare it in `src/framework/mod.rs`. HTTP middleware primitives already live under `src/framework/http/middleware/`.

Treat `src/framework/` as future standalone-crate code: avoid depending on anything in `src/app/`, `src/routes/`, or `src/bootstrap/` from inside `framework/`. The dependency direction is one-way — application code depends on the framework, never the reverse.

## 6. Add HTTP middleware

Middleware runs **before** your handler. Convention: **library / infrastructure** → `Route::layer(...)` (`tower-http`, other `Layer` types); **your policy** → `Route::middleware(...)` with the `Middleware` trait (Axum `middleware::from_fn` under the hood).

### 6.1. Implement middleware (struct)

Create `src/app/http/middleware/<name>.rs` (or extend `trim_strings.rs` as a template). A typical middleware is a unit struct with an `async fn handle(&self, request, next) -> Response`:

```rust
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use kolas::framework::http::middleware::Middleware;

#[derive(Clone, Default)]
pub struct MyMiddleware;

impl Middleware for MyMiddleware {
    async fn handle(&self, request: Request, next: Next) -> Response {
        // Inspect or mutate `request` here if needed.
        next.run(request).await
    }
}
```

Expose it from `src/app/http/middleware/mod.rs` with `pub mod my_middleware;` and `pub use my_middleware::MyMiddleware;`.

### 6.2. Async function middleware (blanket impl)

Function items such as `async fn(req: Request, next: Next) -> Response` also implement `Middleware`, so you can pass them directly to the route builder:

```rust
async fn noop(req: Request, next: Next) -> Response {
    next.run(req).await
}

// ...
Route::new()
    .get("/hello", HelloWorldController::index)
    .middleware(noop)
```

Note: ordinary `|req, next| async move { ... }` closures are often `FnOnce` (they consume `Request`), so they do **not** always satisfy the blanket `Fn` bound. Prefer a named `async fn` or a unit struct.

### 6.3. Register on the `Route` builder

Open `src/routes/api.rs` and chain:

| Method | Typical use | Effect |
|--------|-------------|--------|
| `.layer(L)` | `CompressionLayer`, `CorsLayer`, `TraceLayer`, … | Same as `axum::Router::layer`: applies to the whole router built so far (and routes you add **after** this call). |
| `.middleware(M)` | Types implementing `Middleware` | Same stack position as other global layers; use for app-specific `from_fn` middleware. |
| `.route_middleware(M)` | Scoped auth, etc. | Same as `Router::route_layer`: only routes registered **before** this call. At least one route required or Axum panics. |

Example (matches the project skeleton — app middleware first, then tower-http; on the wire: trace → CORS → compression → `TrimStrings` → handlers):

```rust
use kolas::app::http::middleware::TrimStrings;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

Route::new()
    .get("/", HelloWorldController::index)
    .middleware(TrimStrings)
    .layer(CompressionLayer::new())
    .layer(CorsLayer::permissive())
    .layer(TraceLayer::new_for_http())
    .into_router()
```

### 6.4. What the sample `TrimStrings` middleware does

| Source | When | Behaviour |
|--------|------|-----------|
| Query string | Any HTTP method | Trims decoded parameter values (via `serde_urlencoded`). On parse failure the query is left unchanged. |
| JSON body | `POST` / `PUT` / `PATCH`, `Content-Type: application/json` | Recursively trims string values in `serde_json::Value`. Invalid JSON leaves the body untouched. |
| Form body | Same methods, `application/x-www-form-urlencoded` | Trims each value. Invalid form bodies are left untouched. |
| Keys `password`, `password_confirmation` | JSON, form, query | Values are **not** trimmed. |
| Other bodies | e.g. `multipart/*`, `text/plain` | Body is not buffered or modified. |

Internal design notes: `dev_docs/architecture/middleware.md`.

## 7. Work with a relational database

Kolas ships an async database layer built on top of **SQLx 0.8**. It supports multiple **named connections** declared in `config/database.toml`, with lazy pool initialization and a static `Database` facade that mirrors the `Config` one.

Supported drivers: **PostgreSQL**, **MySQL** (and forks: MariaDB, Percona), **SQLite**. MS SQL Server is intentionally not supported — see `dev_docs/database/improvements.md` (item 9) for the path forward if it's ever required.

### 7.1. Configure a connection

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

Any value can be overridden by an environment variable using the convention from section 2:

| TOML path | ENV variable |
|---|---|
| `database.default` | `DATABASE__DEFAULT` |
| `database.auto_migrate` | `DATABASE__AUTO_MIGRATE` |
| `database.connections.primary.host` | `DATABASE__CONNECTIONS__PRIMARY__HOST` |
| `database.connections.primary.pool.max` | `DATABASE__CONNECTIONS__PRIMARY__POOL__MAX` |
| `database.connections.cache.url` | `DATABASE__CONNECTIONS__CACHE__URL` |

### 7.2. Get a pool in a controller

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

### 7.3. Add a new connection

1. Add a `[connections.<name>]` section to `config/database.toml` with the appropriate `driver` and credentials.
2. Use it from code: `Database::connection("<name>")` (or the typed shortcut for the right driver).

No code changes in the framework, no registration step. New connections are picked up on the next process start.

### 7.4. Migrations

SQL files live in `database/migrations/` (configurable via `database.migrations_path`) and are processed by `sqlx::migrate::Migrator`. Each migration is a **reversible pair** named `<VERSION>_<description>.up.sql` / `<VERSION>_<description>.down.sql` (e.g. `0001_create_users.up.sql` + `0001_create_users.down.sql`). The `up` file applies the change; the `down` file reverts it and is required for rollback to work.

#### Create a migration

```bash
cargo run -- migration:create create_users
# Created ./database/migrations/0001_create_users.up.sql
# Created ./database/migrations/0001_create_users.down.sql
```

The version prefix auto-increments from the highest existing migration (zero-padded to four digits). Fill in the generated files:

```sql
-- 0001_create_users.up.sql
CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT NOT NULL);
```

```sql
-- 0001_create_users.down.sql
DROP TABLE users;
```

#### Run pending migrations

```bash
cargo run -- migration:migrate    # apply all pending migrations to the default connection
```

Equivalent paths from code or at boot:

- **At boot, automatically** — set `auto_migrate = true` in `database.toml` (default is `false`); migrations apply against the default connection right after `Database::install_global()`.
- **Manually, from code** — `kolas::framework::database::migrate_default("./database/migrations").await?;`
- **Against a specific connection** — `migrate("analytics", "./database/migrations").await?;`

Repeated runs are idempotent thanks to the `_sqlx_migrations` tracking table.

#### Roll back the last migration

```bash
cargo run -- migration:rollback   # revert the most recently applied migration
```

Each invocation reverts exactly one migration (the newest applied one) by running its `down.sql`. Run it again to step back further. If nothing has been applied yet it prints `Nothing to roll back.` and exits successfully. From code: `rollback_default("./database/migrations").await?;` or `rollback("analytics", "./database/migrations").await?;` for a specific connection.

### 7.5. Lazy initialization

`Database::install_global()` does **not** open any sockets. The pool for a given connection name is opened on the **first call** to `Database::connection(name)` / `Database::any(name)` / `Database::<driver>(name)`. As a consequence:

- `cargo run` boots successfully even if the database server is currently down.
- The first request that actually touches the DB pays the connection-establishment cost.
- For aggressive warm-up, call the relevant accessor in `bootstrap::app::run()` after `install_global()`.

Architecture rationale, full configuration reference and a backlog of future improvements live in `dev_docs/architecture/database.md` and `dev_docs/database/improvements.md`.

## 8. Add a console command

Console commands live in `src/app/console/commands/`. To add a new command, two steps are needed: create the command file and register it in `src/app/console/commands/mod.rs`. Nothing else needs to be touched.

### 8.1. Create the command file

Create `src/app/console/commands/<name>_command.rs`. A command is a unit struct that implements the `Command` trait from `kolas::framework::console`. Arguments arrive parsed as an `Args` value — read positional arguments with `args.positional(i)` (zero-based), named arguments with `args.get("key")`, and boolean flags with `args.has("flag")`.

Example: `src/app/console/commands/report_command.rs`

```rust
use kolas::framework::console::{Args, BoxFuture, Command};

pub struct ReportCommand;

impl Command for ReportCommand {
    fn name(&self) -> &str {
        "report"
    }

    fn description(&self) -> &str {
        "Generate a report. Usage: report [period] [user]"
    }

    fn execute(&self, args: Args) -> BoxFuture<'_> {
        Box::pin(async move {
            let period = args.positional(0).unwrap_or("daily");
            let user   = args.positional(1).unwrap_or("all");
            println!("Generating {period} report for {user}…");
            Ok(())
        })
    }
}
```

Run it:

```bash
cargo run -- report              # Generating daily report for all…
cargo run -- report weekly alice # Generating weekly report for alice…
```

### 8.2. Register the command in `src/app/console/commands/mod.rs`

This is the only file that needs to change. Declare the module, re-export the struct, and add it to the `all()` vector — `bootstrap/console.rs` picks it up automatically from there:

```rust
pub mod test_command;
pub mod report_command;          // <-- add

pub use test_command::TestCommand;
pub use report_command::ReportCommand;   // <-- add

use crate::framework::console::Command;

pub fn all() -> Vec<Box<dyn Command>> {
    vec![
        Box::new(TestCommand),
        Box::new(ReportCommand),  // <-- add
    ]
}
```

### 8.3. Passing arguments

Everything after the command name is parsed into an `Args` value. The kernel recognizes three shapes:

- `--key=value` — named argument, read with `args.get("key") -> Option<&str>` (use the `=` form; there is no space-separated `--key value`)
- `--flag` — boolean flag, test with `args.has("flag") -> bool`
- bare tokens — positional arguments (in order), read with `args.positional(i) -> Option<&str>`

```rust
fn execute(&self, args: Args) -> BoxFuture<'_> {
    Box::pin(async move {
        let period = args.positional(0).unwrap_or("daily"); // report weekly
        let format = args.get("format").unwrap_or("text");  // --format=json
        let dry_run = args.has("dry-run");                  // --dry-run
        // ...
        Ok(())
    })
}
```

### 8.4. Built-in commands

| Command | Invocation | Description |
|---|---|---|
| `serve` | `cargo run` or `cargo run -- serve` | Start the HTTP server (default when no command is given) |
| `migration:create` | `cargo run -- migration:create <name>` | Create a new `up`/`down` migration file pair |
| `migration:migrate` | `cargo run -- migration:migrate` | Run all pending database migrations |
| `migration:rollback` | `cargo run -- migration:rollback` | Roll back the last applied migration |
| `help` | `cargo run -- help` | List all registered commands with descriptions |

---

# Routing under the hood

`Route` is a thin builder over `axum::Router`. Verbs like `.get` delegate to `Router::route`. `.middleware` and `.layer` delegate to Axum’s `Router::layer`; `.route_middleware` maps to `Router::route_layer`. In `src/routes/api.rs`, **tower-http** layers are chained on the same `Route` builder as application middleware, then `.into_router()` produces the `Router` passed to `axum::serve` in `bootstrap/server.rs` (`HttpServer::run`). Anything Axum supports (extractors, state, error responses) remains available inside controller methods.

# License

Kolas is released under the **MIT License**. See the full text in [`LICENSE`](LICENSE).

Copyright © 2026 Alexandr Mendel.
