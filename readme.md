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
- Default **tower-http** stack is chained on the same `Route` builder: **trace**, permissive **CORS**, **compression** (gzip / brotli / zstd / deflate). Subscriber: `framework::logging::Logging::init()`, called from `bootstrap::app::run()` and `bootstrap::console::run()`; declarative sinks (console/file/database/queue) configured in `config/logging.toml`, tune levels with `RUST_LOG` / `LOG_LEVEL` (see `.env.example`).
- Relational database layer on top of **SQLx 0.8**: multiple named connections in `config/database.toml`, lazy pool initialization, static `Database` facade with both an `AnyPool` path and typed `Pool<Postgres|MySql|Sqlite>` accessors, optional auto-migrate from `database/migrations/`.
- Console command layer: `ConsoleKernel` dispatches CLI arguments to `Command` trait implementations; built-in `serve` and `migrate` commands; register your own commands in `src/bootstrap/console.rs`.
- Dependency injection: `framework::di::Container` — register singletons/transients/per-request-scoped services via `ContainerBuilder` (in `src/app/providers/`), resolve with `Container::resolve` or the `Inject<T, Tag>` Axum extractor; `ScopeMiddleware` backs the per-request lifecycle.

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

## Documentation

The full task-oriented guide is split by topic in [`documentations/`](documentations/):

| Topic | Description |
|---|---|
| [Project structure](documentations/project-structure.md) | Full annotated directory tree |
| [Add a controller](documentations/controllers.md) | Create a controller and register its route |
| [Configure the application](documentations/configuration.md) | TOML config files, env overrides, `.env`, bind address |
| [Write a test](documentations/testing.md) | `tests/feature/` vs `tests/unit/`, conventions |
| [Extend the framework core](documentations/framework-modules.md) | Add a new module under `src/framework/` |
| [Add HTTP middleware](documentations/middleware.md) | `Middleware` trait, `Route::layer` vs `Route::middleware` |
| [Work with a relational database](documentations/database.md) | SQLx connections, migrations, lazy pools |
| [Add a console command](documentations/console-commands.md) | `Command` trait, argument parsing, built-in commands |
| [Routing under the hood](documentations/routing.md) | How `Route` maps onto `axum::Router` |
| [Dependency injection](documentations/dependency-injection.md) | `Container`, `ContainerBuilder`, `Inject<T, Tag>`, lifecycles, `ScopeMiddleware` |

## License

Kolas is released under the **MIT License**. See the full text in [`LICENSE`](LICENSE).

Copyright © 2026 Alexandr Mendel.
