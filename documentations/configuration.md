# Configure the application

Configuration is split into multiple TOML files under `config/`, each becoming a top-level namespace. Values are read from code with a static facade `Config::get(path, default)`, where `path` is a dot-notation address into the merged configuration tree. Any value can be overridden by an environment variable (or `.env` entry) without changing the TOML file.

## 1. Add a new config file

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

## 2. Read values from code

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

## 3. Read a whole subtree as a typed struct

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

## 4. Override values with environment variables

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

## 5. Use `.env` for local development

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

## 6. Resolution order

When the same key is set in multiple places, the last source wins:

1. **TOML file** (`config/<section>.toml`) — base value.
2. **`.env` file** at project root — local override.
3. **Process environment variable** — final override (production deployments, CI, ad-hoc runs like `APP__PORT=8080 cargo run`).

## 7. Static facade vs. instance API

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

## 8. Change the server bind address

The listener reads `app.host` and `app.port` from configuration. To change the bind address, set them in `config/app.toml`:

```toml
host = "0.0.0.0"
port = 8080
```

…or override at runtime without touching files:

```bash
APP__HOST=0.0.0.0 APP__PORT=8080 cargo run
```

Both forms are equivalent; the environment variable wins if both are set.

[← Back to readme](../readme.md)
