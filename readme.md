# Kolas — A Lightweight Rust Web Framework on Top of Axum

Kolas is a lightweight Rust web framework that wraps [Axum](https://github.com/tokio-rs/axum) with an opinionated, MVC-style project layout and a fluent routing API. The goal is to give Rust developers a structured, ready-to-use project layout (controllers, routes, bootstrap) so they can stand up a service in minutes without wiring boilerplate by hand.

The framework core lives under `src/framework/` and is designed to be extracted into a standalone crate later. Application code (your controllers, your routes) lives under `src/app/` and `src/routes/`, following the convention of separating vendor framework code from application code.

## Status

Minimal bootstrap iteration. Currently included:

- Async HTTP server based on Axum 0.8 and Tokio.
- Fluent `Route` builder facade (`Route::new().get(...).post(...).into_router()`).
- Controllers as Rust structs with associated `async fn` handlers.
- A working sample endpoint: `GET /hello` returning `{"payload": "Hello world"}`.

Out of scope for now (planned): config / `.env`, middleware, global error handler, resource routes, CLI commands, database layer.

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
│   └── app.rs                                    # run(): binds TCP listener and starts axum::serve
├── framework/                                    # Framework core (future standalone crate)
│   └── routing/
│       └── route.rs                              # Route builder facade over axum::Router
├── routes/
│   └── api.rs                                    # API route table — register your routes here
└── app/
    └── http/
        └── controllers/
            └── hello_world_controller.rs         # Your controllers go in this directory

tests/
├── feature/                                      # Component / integration tests (HTTP, controllers, end-to-end)
│   ├── mod.rs                                    # entry: declares submodule tests
│   └── hello_world_controller.rs
└── unit/                                         # Focused unit tests of small components
    ├── mod.rs                                    # entry: declares submodule tests
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

## 2. Write a test for your controller

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

In Rust the term "unit test" traditionally means a test colocated with the code under test, inside the source file via `#[cfg(test)] mod tests { ... }`. Tests under `tests/` are technically "integration tests" because each compiles as a separate crate that sees only the **public** API of the `kolas` library. The `tests/unit/` and `tests/feature/` split adopted here is a project convention modeled on PHP/Laravel-style organization: it groups tests by their **scope and intent** (focused-component vs. full-flow), not by Cargo's underlying compilation model. Both directories sit at integration-test level and have full access to `kolas::*` public items.

## 3. Change the server bind address

The listener address is currently hardcoded to `127.0.0.1:3000` in `src/bootstrap/app.rs`. To change it, edit the `tokio::net::TcpListener::bind(...)` call directly. A configuration layer (env / config files) is planned for a future iteration.

## 4. Add a new module to the framework core

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
