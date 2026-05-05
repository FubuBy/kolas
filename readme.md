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

Out of scope for now (planned): middleware, global error handler, resource routes, CLI commands, database layer.

## Requirements

- Rust toolchain compatible with Rust **edition 2024** (`rustc 1.94+` recommended).
- Cargo.

## Quick start

```bash
cargo run
# Listening on http://127.0.0.1:3000
curl http://127.0.0.1:3000/hello
# {"payload":"Hello world"}
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
│   └── app.rs                                    # run(): loads config, binds listener, starts axum::serve
├── framework/                                    # Framework core (future standalone crate)
│   ├── config/                                   # Configuration loader (TOML + env overrides)
│   │   ├── config.rs                             # Config struct + static facade
│   │   └── error.rs                              # Typed loader errors
│   └── routing/
│       └── route.rs                              # Route builder facade over axum::Router
├── routes/
│   └── api.rs                                    # API route table — register your routes here
└── app/
    └── http/
        └── controllers/
            └── hello_world_controller.rs         # Your controllers go in this directory

config/                                           # Application configuration (TOML files)
├── app.toml
├── database.toml
├── cache.toml
└── queue.toml

.env                                              # Local environment overrides (gitignored)
.env.example                                      # Committed template for required env vars

tests/
├── feature/                                      # Component / integration tests (HTTP, controllers, end-to-end)
│   ├── mod.rs                                    # entry: declares submodule tests
│   ├── config_loads_from_directory.rs
│   └── hello_world_controller.rs
└── unit/                                         # Focused unit tests of small components
    ├── mod.rs                                    # entry: declares submodule tests
    ├── config.rs
    └── hello_payload.rs
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

If you want to extend the framework itself (e.g. add middleware, a request-validation layer, an exception handler), create the new sub-module under `src/framework/` and declare it in `src/framework/mod.rs`:

```rust
pub mod routing;
pub mod middleware;   // <-- new core module
```

Treat `src/framework/` as future standalone-crate code: avoid depending on anything in `src/app/`, `src/routes/`, or `src/bootstrap/` from inside `framework/`. The dependency direction is one-way — application code depends on the framework, never the reverse.

---

# Routing under the hood

`Route` is a thin builder over `axum::Router`. Calling `.get(path, handler)` internally invokes `axum::Router::route(path, axum::routing::get(handler))` and returns the builder for chaining. `.into_router()` unwraps the underlying `axum::Router`, which is then served by `axum::serve` in `bootstrap/app.rs`. No global state, no macros — just a typed builder. This means anything Axum supports (extractors, state, error responses) is available unchanged inside controller methods.

# License

Kolas is released under the **MIT License**. See the full text in [`LICENSE`](LICENSE).

Copyright © 2026 Alexandr Mendel.
