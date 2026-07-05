# Write a test for your controller

Tests are split into two directories under `tests/`:

| Directory | Purpose | Typical content |
|---|---|---|
| `tests/feature/` | Component / integration tests | Calling controller handlers, asserting HTTP-shape responses, end-to-end flows |
| `tests/unit/` | Focused unit tests | Pure logic of small components: serialization, validation, helpers |

Each directory has a `mod.rs` entry file (registered as a Cargo test target via `[[test]]` in `Cargo.toml`) that declares its sub-modules. Adding a new test = adding the file plus one `mod` line.

## Adding a feature test

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

## Adding a unit test

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

## Running tests

```bash
cargo test                   # all tests (both binaries)
cargo test --test feature    # only feature tests
cargo test --test unit       # only unit tests
```

## A note on Rust testing conventions

In Rust the term "unit test" traditionally means a test colocated with the code under test, inside the source file via `#[cfg(test)] mod tests { ... }`. Tests under `tests/` are technically "integration tests" because each compiles as a separate crate that sees only the **public** API of the `kolas` library. The `tests/unit/` and `tests/feature/` split adopted here is a project convention borrowed from PHPUnit-style organisation: it groups tests by their **scope and intent** (focused-component vs. full-flow), not by Cargo's underlying compilation model. Both directories sit at integration-test level and have full access to `kolas::*` public items.

[← Back to readme](../readme.md)
